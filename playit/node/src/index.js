// SPDX-License-Identifier: MIT
// Node helper for launching and driving the disco simulator over playit TCP.

import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import net from 'node:net';
import path from 'node:path';
import readline from 'node:readline';

const DEFAULT_HOST = '127.0.0.1';
const DEFAULT_PORT = 0;
const DEFAULT_SCREEN = { width: 800, height: 480 };
const DEFAULT_READY_TIMEOUT_MS = 15_000;
const DEFAULT_COMMAND_TIMEOUT_MS = 5_000;
const INPUT_SETTLE_FRAMES = 2;
const FRAME_POLL_MS = 20;
const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function formatScreen(screen) {
  if (!screen) {
    return `${DEFAULT_SCREEN.width}x${DEFAULT_SCREEN.height}`;
  }
  return `${screen.width}x${screen.height}`;
}

function normalizeKey(key) {
  if (typeof key !== 'string' || key.length === 0) {
    throw new Error('key must be a non-empty string');
  }
  return key;
}

function parseReadyLine(line) {
  const prefix = 'PLAYIT_READY tcp://127.0.0.1:';
  if (!line.startsWith(prefix)) {
    throw new Error(`unexpected ready line: ${line}`);
  }
  const port = Number.parseInt(line.slice(prefix.length), 10);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`invalid ready line port: ${line}`);
  }
  return { host: DEFAULT_HOST, port };
}

function parseStatusLine(line) {
  if (!line.startsWith('STAT:')) {
    throw new Error(`unexpected status response: ${line}`);
  }
  const [tickCount, presentCount] = line.slice(5).split(',').map((value) => {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isInteger(parsed)) {
      throw new Error(`invalid status response: ${line}`);
    }
    return parsed;
  });
  return { tickCount, presentCount };
}

function parseBoundsLine(line) {
  if (!line.startsWith('BOUNDS:')) {
    throw new Error(`unexpected bounds response: ${line}`);
  }
  const [x, y, width, height] = line.slice(7).split(',').map((value) => {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isInteger(parsed)) {
      throw new Error(`invalid bounds response: ${line}`);
    }
    return parsed;
  });
  return { x, y, width, height };
}

class LineChannel {
  constructor(stream, { name, timeoutMs }) {
    this.name = name;
    this.timeoutMs = timeoutMs;
    this.buffer = '';
    this.lines = [];
    this.waiters = [];
    this.closedError = null;

    stream.setEncoding('utf8');
    stream.on('data', (chunk) => this.#onData(chunk));
    stream.on('end', () => this.#finish(new Error(`${this.name} ended unexpectedly`)));
    stream.on('close', () => this.#finish(new Error(`${this.name} closed unexpectedly`)));
    stream.on('error', (error) => this.#finish(error));
  }

  #onData(chunk) {
    this.buffer += chunk;
    for (;;) {
      const newline = this.buffer.indexOf('\n');
      if (newline === -1) {
        return;
      }
      let line = this.buffer.slice(0, newline);
      this.buffer = this.buffer.slice(newline + 1);
      if (line.endsWith('\r')) {
        line = line.slice(0, -1);
      }
      if (this.waiters.length > 0) {
        const waiter = this.waiters.shift();
        waiter.resolve(line);
      } else {
        this.lines.push(line);
      }
    }
  }

  #finish(error) {
    if (this.closedError) {
      return;
    }
    this.closedError = error;
    while (this.waiters.length > 0) {
      const waiter = this.waiters.shift();
      waiter.reject(error);
    }
  }

  nextLine(timeoutMs = this.timeoutMs) {
    if (this.lines.length > 0) {
      return Promise.resolve(this.lines.shift());
    }
    if (this.closedError) {
      return Promise.reject(this.closedError);
    }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        const index = this.waiters.indexOf(waiter);
        if (index !== -1) {
          this.waiters.splice(index, 1);
        }
        reject(new Error(`timed out waiting for ${this.name} output`));
      }, timeoutMs);
      const waiter = {
        resolve: (line) => {
          clearTimeout(timer);
          resolve(line);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        }
      };
      this.waiters.push(waiter);
    });
  }
}

class DiscoSimSession {
  constructor({ child, socket, commandTimeoutMs }) {
    this.child = child;
    this.socket = socket;
    this.commandTimeoutMs = commandTimeoutMs;
    this.lines = new LineChannel(socket, {
      name: 'playit socket',
      timeoutMs: commandTimeoutMs
    });
    this.queue = Promise.resolve();
    this.closed = false;
  }

  #enqueue(task) {
    const run = this.queue.then(task, task);
    this.queue = run.catch(() => {});
    return run;
  }

  #writeCommand(command) {
    if (this.closed) {
      throw new Error('session is closed');
    }
    this.socket.write(`${command}\n`);
  }

  async #readLine() {
    const line = await this.lines.nextLine(this.commandTimeoutMs);
    if (line.startsWith('ERR:')) {
      throw new Error(line.slice(5).trim());
    }
    return line;
  }

  async #expectOk(command) {
    this.#writeCommand(command);
    const line = await this.#readLine();
    if (line !== 'OK') {
      throw new Error(`expected OK for "${command}", got "${line}"`);
    }
  }

  async #statusUnlocked() {
    this.#writeCommand('?');
    return parseStatusLine(await this.#readLine());
  }

  async #waitFramesUnlocked(frames = 1) {
    let status = await this.#statusUnlocked();
    const targetPresent = status.presentCount + frames;
    while (status.presentCount < targetPresent) {
      await delay(FRAME_POLL_MS);
      status = await this.#statusUnlocked();
    }
    return status;
  }

  async #boundsUnlocked(tag) {
    this.#writeCommand(`QB:${tag}`);
    try {
      return parseBoundsLine(await this.#readLine());
    } catch (error) {
      if (error instanceof Error && error.message === 'tag not found') {
        return null;
      }
      throw error;
    }
  }

  async #existsUnlocked(tag) {
    this.#writeCommand(`QE:${tag}`);
    const line = await this.#readLine();
    if (line === 'EXISTS:1') {
      return true;
    }
    if (line === 'EXISTS:0') {
      return false;
    }
    throw new Error(`unexpected exists response: ${line}`);
  }

  status() {
    return this.#enqueue(() => this.#statusUnlocked());
  }

  tick(frames = 1) {
    return this.#enqueue(() => this.#waitFramesUnlocked(frames));
  }

  keyDown(key) {
    return this.#enqueue(async () => {
      await this.#expectOk(`KD:${normalizeKey(key)}`);
      return this.#waitFramesUnlocked(INPUT_SETTLE_FRAMES);
    });
  }

  keyUp(key) {
    return this.#enqueue(async () => {
      await this.#expectOk(`KU:${normalizeKey(key)}`);
      return this.#waitFramesUnlocked(INPUT_SETTLE_FRAMES);
    });
  }

  pointerDown(x, y) {
    return this.#enqueue(async () => {
      await this.#expectOk(`PD${x},${y}`);
      return this.#waitFramesUnlocked(INPUT_SETTLE_FRAMES);
    });
  }

  pointerMove(x, y) {
    return this.#enqueue(async () => {
      await this.#expectOk(`PM${x},${y}`);
      return this.#waitFramesUnlocked(INPUT_SETTLE_FRAMES);
    });
  }

  pointerUp(x, y) {
    return this.#enqueue(async () => {
      await this.#expectOk(`PU${x},${y}`);
      return this.#waitFramesUnlocked(INPUT_SETTLE_FRAMES);
    });
  }

  dumpRect({ x, y, width, height, frames = 1 }) {
    return this.#enqueue(async () => {
      this.#writeCommand(`D${x},${y},${width},${height},${frames}`);
      const queued = await this.#readLine();
      if (queued !== 'DUMP:queued') {
        throw new Error(`unexpected dump response: ${queued}`);
      }

      const capturedFrames = [];
      let currentFrame = null;
      for (;;) {
        const line = await this.#readLine();
        if (line === 'F') {
          if (currentFrame !== null) {
            capturedFrames.push(currentFrame);
          }
          currentFrame = [];
          continue;
        }
        if (line === 'END') {
          if (currentFrame !== null) {
            capturedFrames.push(currentFrame);
          }
          return { x, y, width, height, frames: capturedFrames };
        }
        if (currentFrame === null) {
          throw new Error(`unexpected dump payload before frame marker: ${line}`);
        }
        currentFrame.push(
          line
            .split(/\s+/)
            .filter(Boolean)
            .map((pixel) => Number.parseInt(pixel, 16) >>> 0)
        );
      }
    });
  }

  widget(tag) {
    return new WidgetLocator(this, tag);
  }

  async bounds(tag) {
    return this.#enqueue(() => this.#boundsUnlocked(tag));
  }

  async exists(tag) {
    return this.#enqueue(() => this.#existsUnlocked(tag));
  }

  async tapTag(tag) {
    return this.#enqueue(async () => {
      const bounds = await this.#boundsUnlocked(tag);
      if (!bounds || bounds.width <= 0 || bounds.height <= 0) {
        throw new Error(`widget "${tag}" is not visible`);
      }
      const centerX = bounds.x + Math.floor(bounds.width / 2);
      const centerY = bounds.y + Math.floor(bounds.height / 2);
      await this.#expectOk(`T@${tag}:${centerX},${centerY}`);
      return this.#waitFramesUnlocked(INPUT_SETTLE_FRAMES);
    });
  }

  async close() {
    this.closed = true;
    this.socket.end();
    this.socket.destroy();
    if (this.child && this.child.exitCode === null && this.child.signalCode === null) {
      this.child.kill();
      try {
        await once(this.child, 'exit');
      } catch {
        // Ignore shutdown races on process teardown.
      }
    }
  }
}

class WidgetLocator {
  constructor(session, tag) {
    this.session = session;
    this.tag = tag;
  }

  tap() {
    return this.session.tapTag(this.tag);
  }

  bounds() {
    return this.session.bounds(this.tag);
  }

  exists() {
    return this.session.exists(this.tag);
  }

  async isVisible() {
    const bounds = await this.bounds();
    return !!bounds && bounds.width > 0 && bounds.height > 0;
  }

  async dump(options = {}) {
    const bounds = await this.bounds();
    if (!bounds || bounds.width <= 0 || bounds.height <= 0) {
      throw new Error(`widget "${this.tag}" is not visible`);
    }
    return this.session.dumpRect({
      x: bounds.x,
      y: bounds.y,
      width: options.width ?? bounds.width,
      height: options.height ?? bounds.height,
      frames: options.frames ?? 1
    });
  }
}

async function waitForReadyLine(child, timeoutMs) {
  if (!child.stdout) {
    throw new Error('simulator stdout was not piped');
  }

  const output = readline.createInterface({ input: child.stdout });
  let stderr = '';
  if (child.stderr) {
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
  }

  return await new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      output.close();
      reject(new Error('timed out waiting for PLAYIT_READY'));
    }, timeoutMs);

    const onExit = (code, signal) => {
      clearTimeout(timer);
      output.close();
      reject(
        new Error(
          `disco simulator exited before becoming ready (code=${code}, signal=${signal})\n${stderr}`
        )
      );
    };

    child.once('exit', onExit);
    output.on('line', (line) => {
      if (!line.startsWith('PLAYIT_READY ')) {
        return;
      }
      clearTimeout(timer);
      child.removeListener('exit', onExit);
      output.close();
      resolve(line);
    });
  });
}

async function connectSocket({ host, port, timeoutMs }) {
  const socket = net.createConnection({ host, port });
  const connectPromise = once(socket, 'connect');
  const errorPromise = once(socket, 'error').then(([error]) => {
    throw error;
  });
  await Promise.race([
    connectPromise,
    errorPromise,
    delay(timeoutMs).then(() => {
      throw new Error('timed out connecting to the playit socket');
    })
  ]);
  return socket;
}

export async function launchDiscoSim(options = {}) {
  const commandTimeoutMs = options.commandTimeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS;
  const readyTimeoutMs = options.readyTimeoutMs ?? DEFAULT_READY_TIMEOUT_MS;
  const playitPort = options.playitPort ?? DEFAULT_PORT;
  const automationHeadless = options.automationHeadless ?? true;
  const screen = formatScreen(options.screen);
  const binaryPath = options.binaryPath ?? process.env.RLVGL_DISCO_SIM_BIN;
  const childEnv = { ...process.env, ...(options.env ?? {}) };

  const simulatorArgs = [`--screen=${screen}`, '--playit-port', String(playitPort)];
  if (automationHeadless) {
    simulatorArgs.unshift('--automation-headless');
  }
  if (Array.isArray(options.extraArgs) && options.extraArgs.length > 0) {
    simulatorArgs.push(...options.extraArgs);
  }

  const child = binaryPath
    ? spawn(binaryPath, simulatorArgs, {
        cwd: options.cwd ?? REPO_ROOT,
        env: childEnv,
        stdio: ['ignore', 'pipe', 'pipe']
      })
    : spawn(
        'cargo',
        [
          'run',
          '-p',
          'rlvgl-example-disco-sim',
          '--bin',
          'rlvgl-disco-sim',
          '--',
          ...simulatorArgs
        ],
        {
          cwd: options.cwd ?? REPO_ROOT,
          env: childEnv,
          stdio: ['ignore', 'pipe', 'pipe']
        }
      );

  const readyLine = await waitForReadyLine(child, readyTimeoutMs);
  const socket = await connectSocket({
    ...parseReadyLine(readyLine),
    timeoutMs: commandTimeoutMs
  });

  return new DiscoSimSession({ child, socket, commandTimeoutMs });
}
