"""CRATES-CI §5: frozen publishing refs, manual recovery and bounded failures.

Uses local Git fixtures and fake Cargo/registry commands; never uploads.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[1]


def workflow(name):
    return yaml.safe_load((ROOT / '.github/workflows' / name).read_text())


def triggers(doc):
    # PyYAML's YAML 1.1 mode reads an unquoted GitHub `on` as True.
    return doc.get('on', doc.get(True))


def run_git(path, *args):
    return subprocess.check_output(['git', *args], cwd=path, stderr=subprocess.DEVNULL).decode().strip()


def init_git(path):
    run_git(path, 'init', '-q')
    run_git(path, 'config', 'user.name', 'Local workflow fixture')
    run_git(path, 'config', 'user.email', 'fixture@example.invalid')
    run_git(path, 'config', 'commit.gpgsign', 'false')


class PublishWorkflows(unittest.TestCase):
    def test_frozen_graph_and_authentication_boundary(self):
        recovery = workflow('publish-continue.yml')
        self.assertEqual(set(triggers(recovery)), {'workflow_dispatch'})
        jobs = recovery['jobs']
        self.assertEqual(set(jobs['publish']['needs']), {'resolve-release', 'gate-p'})
        self.assertNotIn('if', jobs['publish'])  # Default successful-needs guard.
        self.assertNotIn('continue-on-error', jobs['gate-p'])
        ref = '${{ needs.resolve-release.outputs.tag_sha }}'
        for job in ['gate-p', 'publish']:
            checkouts = [s for s in jobs[job]['steps'] if s.get('uses', '').startswith('actions/checkout@')]
            self.assertEqual(checkouts[-1]['with']['ref'], ref)
            self.assertEqual(checkouts[-1]['with']['submodules'], 'recursive')
        for job in ['resolve-release', 'gate-p']:
            self.assertEqual(jobs[job]['permissions'], {'contents': 'read'})
        publish = jobs['publish']
        self.assertEqual(publish['environment'], 'release')
        self.assertEqual(publish['permissions'], {'contents': 'read', 'id-token': 'write'})
        steps = publish['steps']
        names = [s.get('name') for s in steps]
        self.assertLess(names.index('Verify frozen release source'), names.index('Authenticate with crates.io'))
        self.assertLess(names.index('Preserve publish helper from workflow revision'), names.index('Checkout frozen release source'))
        required = {
            'bash consumers/lib-smoke/gate_p.sh',
            'bash consumers/creator-cli/smoke.sh',
            'bash consumers/user-sim/gate_p.sh',
        }
        self.assertTrue(required.issubset({s.get('run') for s in jobs['gate-p']['steps']}))
        primary = workflow('publish.yml')
        self.assertNotIn('schedule', triggers(primary))
        self.assertEqual(primary['jobs']['publish']['needs'], 'gate-p')
        for job in ['gate-p', 'publish']:
            checkout = next(s for s in primary['jobs'][job]['steps'] if s.get('uses', '').startswith('actions/checkout@'))
            self.assertEqual(checkout['with']['ref'], '${{ github.sha }}')
        registry = triggers(workflow('gate-r.yml'))
        self.assertEqual(registry['schedule'], [{'cron': '30 16 * * *'}])
        self.assertEqual(set(registry['workflow_run']['workflows']), {'Publish', 'Publish Continue'})

    def test_actual_resolver_and_source_guards(self):
        jobs = workflow('publish-continue.yml')['jobs']
        resolver = next(s['run'] for s in jobs['resolve-release']['steps'] if s.get('id') == 'tags')
        with tempfile.TemporaryDirectory(prefix='rlvgl-release-ref-') as tmp:
            path = Path(tmp)
            init_git(path)
            source = path / 'source.txt'
            for version in ['0.1.0', '0.2.0']:
                source.write_text(version)
                run_git(path, 'add', 'source.txt')
                run_git(path, 'commit', '-qm', version)
                run_git(path, 'tag', '-a', f'v{version}', '-m', version)
            expected = run_git(path, 'rev-parse', 'v0.2.0^{commit}')
            source.write_text('later workflow source')
            run_git(path, 'commit', '-qam', 'later workflow')
            output = path.parent / (path.name + '-output')
            try:
                env = dict(os.environ, GITHUB_OUTPUT=str(output))
                result = subprocess.run(['bash', '-eo', 'pipefail', '-c', resolver], cwd=path, env=env, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                resolved = dict(line.split('=', 1) for line in output.read_text().splitlines())
                self.assertEqual(resolved['tag_sha'], expected)
                self.assertNotEqual(resolved['workflow_sha'], expected)
                self.assertEqual(resolved['base_sha'], run_git(path, 'rev-parse', 'v0.1.0^{commit}'))
            finally:
                output.unlink(missing_ok=True)
            for doc in [workflow('publish-continue.yml'), workflow('publish.yml')]:
                guard = next(s['run'] for s in doc['jobs']['publish']['steps'] if s.get('name') == 'Verify frozen release source')
                env = dict(os.environ, EXPECTED_RELEASE_SHA=expected)
                self.assertNotEqual(subprocess.run(['bash', '-e', '-c', guard], cwd=path, env=env).returncode, 0)
                run_git(path, 'checkout', '--detach', expected)
                self.assertEqual(subprocess.run(['bash', '-e', '-c', guard], cwd=path, env=env).returncode, 0)
                source.write_text('dirty tagged source')
                self.assertNotEqual(subprocess.run(['bash', '-e', '-c', guard], cwd=path, env=env).returncode, 0)
                source.write_text('0.2.0')
                run_git(path, 'checkout', '--detach', resolved['workflow_sha'])

    def publisher_fixture(self, existing='', failure=''):
        with tempfile.TemporaryDirectory(prefix='rlvgl-publisher-') as tmp:
            path = Path(tmp)
            init_git(path)
            bridge = path / 'crates/ratatui-rlvgl/Cargo.toml'
            bridge.parent.mkdir(parents=True)
            bridge.write_text('[package]\nname="ratatui-rlvgl"\nversion="0.1.2"\n')
            run_git(path, 'add', '.')
            run_git(path, 'commit', '-qm', 'baseline')
            base = run_git(path, 'rev-parse', 'HEAD')
            names = ['api', 'core', 'rlvgl-decomp', 'i18n', 'playit', 'platform', 'audio-meters-core', 'widgets', 'ui']
            crates = [name if name.startswith('rlvgl-') else 'rlvgl-' + name for name in names]
            for name in names:
                source = path / name / 'src/lib.rs'
                source.parent.mkdir(parents=True)
                source.write_text('// controlled changed package\n')
            run_git(path, 'add', '.')
            run_git(path, 'commit', '-qm', 'nine packages')
            tools = path / 'fake-tools'
            tools.mkdir()
            cargo = tools / 'cargo'
            cargo.write_text('''#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
args=sys.argv[1:]
if args[0]=='metadata':
 names=['ratatui-rlvgl'] if '--manifest-path' in args else json.loads(os.environ['FIXTURE_CRATES'])
 print(json.dumps({'packages':[{'name':n,'version':'0.1.2' if n=='ratatui-rlvgl' else '0.2.7'} for n in names]}))
elif args[0]=='publish':
 name=args[args.index('-p')+1]
 with open(os.environ['FIXTURE_ATTEMPTS'],'a') as f:f.write(name+'\\n')
 if name==os.environ['FIXTURE_FAILURE']:
  print('429 Too Many Requests: Please try again after Wed, 07 Oct 2026 00:00:00 GMT and see.')
  sys.exit(1)
else:sys.exit(90)
''')
            curl = tools / 'curl'
            curl.write_text('''#!/usr/bin/env python3
import os,sys
name=sys.argv[-1].split('/')[-2]
print('200' if name==os.environ['FIXTURE_EXISTING'] else '404',end='')
''')
            cargo.chmod(0o755)
            curl.chmod(0o755)
            attempts = path / 'attempts.txt'
            env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'], INDEX_WAIT_SECONDS='0',
                       DRY_RUN='0', CARGO_REGISTRY_TOKEN='local-fixture-only', FIXTURE_CRATES=json.dumps([*crates, 'ratatui-rlvgl']),
                       FIXTURE_ATTEMPTS=str(attempts), FIXTURE_EXISTING=existing, FIXTURE_FAILURE=failure)
            result = subprocess.run(['bash', str(ROOT / 'scripts/publish_changed.sh'), base], cwd=path, env=env, capture_output=True, text=True)
            return crates, attempts.read_text().splitlines() if attempts.exists() else [], result

    def test_publisher_processes_more_than_five_and_skips_published(self):
        crates, attempts, result = self.publisher_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, crates)
        crates, attempts, result = self.publisher_fixture(existing='rlvgl-core')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, [name for name in crates if name != 'rlvgl-core'])

    def test_registry_throttle_stops_without_publishing_dependents(self):
        crates, attempts, result = self.publisher_fixture(failure='rlvgl-platform')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(attempts, crates[:6])
        self.assertIn('Retry after Wed, 07 Oct 2026 00:00:00 GMT', result.stderr)


    def owned_bridge_selector_fixture(self, change='owned', duplicate=False):
        """Run the real selector with disposable Git source and a metadata-only Cargo stub."""
        with tempfile.TemporaryDirectory(prefix='rlvgl-owned-selector-') as tmp:
            path = Path(tmp)
            init_git(path)
            native = path / 'vendor/ratatui'
            native.mkdir(parents=True)
            init_git(native)
            (native / 'README.md').write_text('Actual native fixture baseline.\n')
            run_git(native, 'add', '.')
            run_git(native, 'commit', '-qm', 'native baseline')
            bridge = path / 'crates/ratatui-rlvgl'
            (bridge / 'src').mkdir(parents=True)
            (bridge / 'Cargo.toml').write_text('[package]\nname="ratatui-rlvgl"\nversion="0.1.2"\n')
            (bridge / 'src/lib.rs').write_text('// Actual owned baseline.\n')
            (path / 'Cargo.toml').write_text('[package]\nname="rlvgl"\nversion="0.2.8"\n')
            run_git(path, 'add', '.')
            run_git(path, 'commit', '-qm', 'owned baseline')
            base = run_git(path, 'rev-parse', 'HEAD')
            if change == 'owned':
                (bridge / 'src/lib.rs').write_text('// Actual owned change.\n')
            elif change == 'gitlink':
                (native / 'README.md').write_text('Actual native dependency change.\n')
                run_git(native, 'commit', '-qam', 'native dependency')
                run_git(path, 'add', 'vendor/ratatui')
            elif change == 'unrelated':
                (path / 'unrelated.md').write_text('No publishable package change.\n')
            else:
                raise AssertionError(change)
            tools = path / 'fake-tools'
            tools.mkdir()
            cargo = tools / 'cargo'
            cargo.write_text("""#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
with open(os.environ['FIXTURE_METADATA_CALLS'],'a') as f:f.write(json.dumps(sys.argv[1:])+'\\n')
if sys.argv[1]!='metadata':raise SystemExit(90)
packages=[{'name':'ratatui-rlvgl','version':'0.1.2'},{'name':'rlvgl','version':'0.2.8'}]
if os.environ['FIXTURE_DUPLICATE']=='1':packages.append(packages[0])
print(json.dumps({'packages':packages}))
""")
            curl = tools / 'curl'
            curl.write_text('#!/bin/sh\nexit 91\n')
            cargo.chmod(0o755)
            curl.chmod(0o755)
            calls = path / 'metadata-calls.jsonl'
            env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                       DRY_RUN='1', FIXTURE_DUPLICATE='1' if duplicate else '0',
                       FIXTURE_METADATA_CALLS=str(calls))
            env.pop('CARGO_REGISTRY_TOKEN', None)
            result = subprocess.run(['bash', str(ROOT / 'scripts/publish_changed.sh'), base],
                                    cwd=path, env=env, capture_output=True, text=True)
            selected = [line.strip()[2:] for line in result.stdout.splitlines() if line.strip().startswith('- ')]
            return selected, [json.loads(line) for line in calls.read_text().splitlines()], result

    def test_owned_bridge_selector_and_native_dependency_trigger(self):
        for change in ['owned', 'gitlink']:
            with self.subTest(change=change):
                selected, calls, result = self.owned_bridge_selector_fixture(change)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(selected, ['ratatui-rlvgl'])
                self.assertEqual(len(calls), 1)  # Root metadata only; no legacy nested append.
                self.assertNotIn('--manifest-path', calls[0])
                self.assertIn('skipping cargo publish', result.stdout)
        selected, calls, result = self.owned_bridge_selector_fixture('unrelated')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(selected, [])

    def test_duplicate_owned_bridge_metadata_is_refused(self):
        selected, calls, result = self.owned_bridge_selector_fixture(duplicate=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(selected, [])
        self.assertEqual(len(calls), 1)


if __name__ == '__main__':
    unittest.main(verbosity=2)
