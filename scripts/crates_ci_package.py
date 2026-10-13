"""CRATES-CI-00 §6: bounded real graph admission and extracted-only resolution.

Cargo owns inventory, normalization, archives and temporary lock updates. The
two real descriptor/member contexts enumerated for the owned bridge migration are permitted
only during archive preparation. Unenumerated context fails without rewriting
an original manifest or assigning outer Git identity to nested source.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

GENERATED = {'Cargo.toml.orig', 'Cargo.lock', '.cargo_vcs_info.json'}


class GateFailure(RuntimeError):
    """A named packaging boundary failed; no admission relaxation is attempted."""


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def lock_snapshot(paths: list[Path]) -> dict[Path, str | None]:
    return {path: sha(path) if path.is_file() else None for path in paths}


def preserve_locks(snapshot: dict[Path, str | None]) -> None:
    for path, digest in snapshot.items():
        if (sha(path) if path.is_file() else None) != digest:
            raise GateFailure(f'P-SOURCE-LOCK: {path}')


def run(command: list[str], *, cwd: Path, **kwargs) -> subprocess.CompletedProcess:
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise GateFailure(f"P-CARGO: {' '.join(command)}\n{result.stdout}{result.stderr}")
    return result


def inherited(value) -> bool:
    if isinstance(value, dict):
        return value.get('workspace') is True or any(inherited(v) for v in value.values())
    if isinstance(value, list):
        return any(inherited(v) for v in value)
    return False


def local_graph_audit(metadata: dict, allowed: set[Path]) -> None:
    """Reject original/admission routes, including a symlink hiding such a route."""
    for package in metadata['packages']:
        source = package.get('source')
        if source is None:
            manifest = Path(package['manifest_path']).resolve()
            if manifest not in allowed:
                raise GateFailure(f"P-ADMISSION-LEAK: {package['name']}: {manifest}")
        elif not source.startswith('registry+'):
            raise GateFailure(f"P-RESOLVE-SOURCE: {package['name']}: {source}")


def vcs_audit(vcs: dict, head: str, path: str, dirty: bool) -> None:
    """Compare Cargo's actual generated nested/root provenance; never rewrite it."""
    if vcs['git']['sha1'] != head or vcs['path_in_vcs'] != path:
        raise GateFailure(f'P-VCS: {vcs}')
    if dirty and not vcs['git'].get('dirty'):
        raise GateFailure('P-VCS-DIRTY: dirty source overlay concealed')


def untracked_member_audit(root: Path, files: list[str], manifests: list[Path]) -> None:
    """Refuse omitted untracked payload, including ignored explicit includes."""
    for relative in filter(None, files):
        source = root / relative
        if source.name != 'Cargo.lock' and any(source.is_relative_to(p.parent) for p in manifests):
            raise GateFailure(f'P-WORKSPACE-UNTRACKED: selected member source {source}')


class WorkspacePolicy:
    """Finite original descriptor, member, target and shared-file authority."""

    def __init__(self, repo: Path, policy: dict, metadata: list[dict]):
        self.repo = repo
        self.workspaces = policy['workspaces']
        self.members = {}
        self.descriptors = {}
        for context in self.workspaces:
            descriptor = repo / context['descriptor']
            if sha(descriptor) != context['descriptor_sha256']:
                raise GateFailure(f'P-DESCRIPTOR-CHANGED: {descriptor}')
            self.descriptors[descriptor.resolve()] = context
            expected = {str(repo / p['manifest']) for p in context['members']}
            found = set()
            for document in metadata:
                if Path(document['workspace_root']).resolve() == descriptor.parent.resolve():
                    found |= {p['manifest_path'] for p in document['packages']
                              if p['id'] in document['workspace_members']}
            if found != expected:
                raise GateFailure(f'P-WORKSPACE-MEMBERS: {descriptor}: {sorted(found ^ expected)}')
            actual = {p['manifest_path']: p for d in metadata for p in d['packages']}
            for member in context['members']:
                manifest = repo / member['manifest']
                if sha(manifest) != member['manifest_sha256']:
                    raise GateFailure(f'P-MEMBER-CHANGED: {manifest}')
                targets = sorted(str(Path(t['src_path']).relative_to(repo))
                                 for t in actual[str(manifest)]['targets'])
                if targets != member['targets']:
                    raise GateFailure(f'P-WORKSPACE-TARGETS: {manifest}')
                for path in targets:
                    if not (repo / path).is_file():
                        raise GateFailure(f'P-WORKSPACE-TARGETS: missing {repo / path}')
                for shared in member['shared_files']:
                    if sha(repo / shared['path']) != shared['sha256']:
                        raise GateFailure(f"P-WORKSPACE-SHARED: {shared['path']}")
                self.members[manifest.resolve()] = (context, member)

    def context_for(self, manifest: Path) -> dict:
        found = self.members.get(manifest.resolve())
        if found is None:
            raise GateFailure(f'P-ADMISSION-WORKSPACE: unenumerated member {manifest}')
        return found[0]


class PackageInput:
    """An immutable payload projection plus separately recorded dev graph input."""

    def __init__(self, repo: Path, graph: Path, current: dict, packages: list[dict],
                 ordered: list[str], staged: dict[str, Path], *, policy=None, patch=None):
        self.repo = repo.resolve()
        self.graph = graph
        self.current = current
        self.packages = {Path(p['manifest_path']).resolve(): p for p in packages}
        self.order = ordered
        self.staged = staged
        self.current_position = ordered.index(current['name'])
        self.records: dict[str, dict] = {}
        self.aliases: dict[str, str] = {}
        self.policy = policy
        self.patch = patch
        self.context_records = {}
        self.active_contexts = set()
        self.source_files = {}
        self.context_edges = []

    def destination(self, manifest: Path) -> Path:
        try:
            relative = manifest.parent.resolve().relative_to(self.repo)
        except ValueError as error:
            raise GateFailure(f'P-ADMISSION-OUTSIDE: {manifest}') from error
        return self.graph / relative

    def inventory(self, package: dict) -> list[str]:
        manifest = Path(package['manifest_path'])
        if self.policy is not None and manifest.resolve() in self.policy.members:
            context = self.policy.context_for(manifest)
            if context['descriptor'] in self.active_contexts:
                projected = self.destination(manifest) / 'Cargo.toml'
                result = run(['cargo', '--config', str(self.patch), 'package', '--list',
                              '--allow-dirty', '--manifest-path', str(projected)], cwd=self.graph)
                return result.stdout.splitlines()
        result = run(['cargo', 'package', '--list', '--allow-dirty', '--locked',
                      '--manifest-path', str(manifest)], cwd=self.repo)
        return result.stdout.splitlines()

    def predecessor(self, package: dict) -> bool:
        name = package['name']
        if name not in self.order or self.order.index(name) >= self.current_position:
            return False
        target = self.staged.get(name)
        if target is None or not (target / 'Cargo.toml').is_file():
            raise GateFailure(f"P-MISSING-PREDECESSOR: {self.current['name']} requires {name}")
        alias = self.destination(Path(package['manifest_path']))
        if alias.exists() or alias.is_symlink():
            if not alias.is_symlink() or alias.resolve() != target.resolve():
                raise GateFailure(f'P-ADMISSION-ALIAS: {alias}')
        else:
            alias.parent.mkdir(parents=True, exist_ok=True)
            alias.symlink_to(target.resolve(), target_is_directory=True)
        self.aliases[name] = str(target.resolve())
        self.refresh_patch()
        return True

    def refresh_patch(self) -> None:
        """One lexical identity for an extracted predecessor during preparation."""
        if self.patch is None:
            return
        entries = ['[patch.crates-io]']
        for name, target in self.staged.items():
            if name in self.aliases:
                package = next(p for p in self.packages.values() if p['name'] == name)
                target = self.destination(Path(package['manifest_path']))
                if target.resolve() != self.staged[name].resolve():
                    raise GateFailure(f'P-ADMISSION-ALIAS: {target}')
            entries.append(f'{name} = {{ path = {json.dumps(str(target))} }}')
        self.patch.write_text('\n'.join(entries) + '\n')

    def copy_real(self, source: Path, target: Path) -> None:
        source = source.resolve()
        try:
            source.relative_to(self.repo)
        except ValueError as error:
            raise GateFailure(f'P-ADMISSION-OUTSIDE: {source}') from error
        if not source.is_file():
            raise GateFailure(f'P-INCLUDE: missing real context file {source}')
        if target.exists() and target.read_bytes() != source.read_bytes():
            raise GateFailure(f'P-CONTEXT-COLLISION: {target}')
        if target.exists():
            self.source_files[str(source)] = sha(source)
            return  # In particular, never write through an extracted alias.
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        self.source_files[str(source)] = sha(source)

    def index_paths(self, paths: list[Path]) -> None:
        """Preserve selected tracked/ignored bytes in the nearest temporary Git index."""
        groups = {}
        for path in paths:
            if not path.resolve().is_relative_to(self.graph.resolve()):
                continue  # Extracted predecessors remain untouched.
            root = Path(run(['git', 'rev-parse', '--show-toplevel'], cwd=path.parent).stdout.strip())
            if not root.resolve().is_relative_to(self.graph.resolve()):
                raise GateFailure(f'P-INDEX-OUTSIDE: {path}')
            groups.setdefault(root, []).append(str(path.resolve().relative_to(root.resolve())))
        for root, relative in groups.items():
            run(['git', 'add', '-f', '--', *relative], cwd=root)

    def index_payloads(self) -> None:
        paths = [self.destination(Path(record['manifest'])) / relative
                 for record in self.records.values() for relative in record['payload_sha256']]
        self.index_paths(paths)

    def attach_nested_git(self, source: Path) -> None:
        """Attach an actual nested clone while retaining earlier extracted aliases."""
        source = source.resolve()
        nested = self.graph / source.relative_to(self.repo)
        if nested.exists():
            for item in nested.iterdir():
                allowed = any(item == self.destination(Path(p['manifest_path'])) and
                              item.is_symlink() and item.resolve() == Path(self.aliases[p['name']]).resolve()
                              for p in self.packages.values() if p['name'] in self.aliases)
                if not allowed:
                    raise GateFailure(f'P-NESTED-COLLISION: {item}')
            clone = self.graph.parent / ('nested-' + source.name)
            run(['git', 'clone', '--shared', '--no-checkout', str(source), str(clone)], cwd=self.graph)
            shutil.move(str(clone / '.git'), str(nested / '.git'))
            clone.rmdir()
        else:
            run(['git', 'clone', '--shared', '--no-checkout', str(source), str(nested)], cwd=self.graph)

    def prepare_workspace(self, context: dict) -> None:
        descriptor = context['descriptor']
        if descriptor in self.active_contexts:
            return
        descriptor_path = self.repo / descriptor
        member_paths = {self.repo / p['manifest']: p for p in context['members']}
        # A genuine nested checkout gives Cargo the nested commit/path/dirty
        # provenance. Neither original manifests nor VCS JSON are fabricated.
        if descriptor_path.parent != self.repo:
            self.attach_nested_git(descriptor_path.parent)
        self.active_contexts.add(descriptor)
        self.copy_real(descriptor_path, self.graph / descriptor)
        seed = descriptor_path.parent / 'Cargo.lock'
        if seed.is_file():
            shutil.copy2(seed, self.graph / descriptor_path.parent.relative_to(self.repo) / 'Cargo.lock')
        initial = {}
        if descriptor_path.parent == self.repo:
            # Root source inventory works in its real source workspace. Preserve
            # the manifest exactly and replace prior member routes with archives.
            for manifest in member_paths:
                package = self.packages[manifest.resolve()]
                if self.predecessor(package):
                    continue
                result = run(['cargo', 'package', '--list', '--allow-dirty', '--locked',
                              '--manifest-path', str(manifest)], cwd=self.repo)
                for relative in result.stdout.splitlines():
                    if relative in GENERATED:
                        continue
                    source = manifest.parent / relative
                    target = self.destination(manifest) / relative
                    self.copy_real(source, target)
                    initial[str(target)] = str(source.resolve())
        else:
            # Enumerated official native member files load its genuine workspace.
            # Cargo then selects inventories with extracted RLVGL predecessors
            # already available in the separate root context.
            untracked = run(['git', 'ls-files', '--others', '-z'],
                            cwd=descriptor_path.parent).stdout.split('\0')
            untracked_member_audit(descriptor_path.parent, untracked, list(member_paths))
            files = run(['git', 'ls-files', '--cached', '-z'], cwd=descriptor_path.parent).stdout.split('\0')
            for relative in filter(None, files):
                source = descriptor_path.parent / relative
                owners = [p for p in member_paths if source.is_relative_to(p.parent)]
                if not owners:
                    continue
                owner = max(owners, key=lambda p: len(p.parts))
                package = self.packages[owner.resolve()]
                if self.predecessor(package):
                    continue
                target = self.graph / source.relative_to(self.repo)
                self.copy_real(source, target)
                initial[str(target)] = str(source.resolve())
        for member in context['members']:
            for shared in member['shared_files']:
                source = self.repo / shared['path']
                self.copy_real(source, self.graph / shared['path'])
                initial[str(self.graph / shared['path'])] = str(source)
        # Cargo's inventory selection distinguishes tracked ignored files and
        # tracked manifests. Preserve this in the genuine temporary Git root
        # BEFORE projected inventories, especially for the nested context.
        self.index_paths([Path(target) for target in initial] + [self.graph / descriptor])
        # Route all path prerequisites before any inventory resolution. Only
        # these finite member contexts or already accepted dev closure may load.
        for manifest in member_paths:
            package = self.packages[manifest.resolve()]
            if self.predecessor(package):
                continue
            for dependency in package['dependencies']:
                if not dependency.get('path'):
                    continue
                dep_manifest = (Path(dependency['path']) / 'Cargo.toml').resolve()
                dep_package = self.packages.get(dep_manifest)
                if dep_package is None:
                    raise GateFailure(f'P-ADMISSION-UNKNOWN: {dep_manifest}')
                if self.predecessor(dep_package):
                    continue
                if dep_manifest not in {p.resolve() for p in member_paths}:
                    other = self.policy.members.get(dep_manifest)
                    if other is None:
                        raise GateFailure(f'P-WORKSPACE-CLOSURE: additional source {dep_manifest}')
                    self.context_edges.append({'from': str(manifest), 'to': str(dep_manifest),
                                               'descriptor': other[0]['descriptor']})
                    self.prepare_workspace(other[0])
        inventories = {}
        keep = {str(self.graph / descriptor)}
        keep |= {str(self.graph / p['manifest']) for p in context['members']}
        keep |= {str(self.graph / t) for p in context['members'] for t in p['targets']}
        keep |= {str(self.graph / f['path']) for p in context['members'] for f in p['shared_files']}
        for manifest in member_paths:
            package = self.packages[manifest.resolve()]
            if package['name'] in self.aliases:
                continue
            files = self.inventory(package)
            inventories[package['name']] = files
            for relative in files:
                if relative in GENERATED:
                    continue
                keep.add(str(self.destination(manifest) / relative))
        for target in initial:
            if target not in keep:
                Path(target).unlink()
        self.context_records[descriptor] = {'descriptor_sha256': sha(descriptor_path),
                                           'member_inventories': inventories,
                                           'prearchive_source_files': dict(self.source_files)}

    def add(self, package: dict, *, closure: bool = False) -> None:
        name = package['name']
        if name in self.records:
            return
        manifest = Path(package['manifest_path']).resolve()
        # Registry-spelled unpublished predecessors also need extracted input.
        # This check happens before inventory resolution, so registry/source
        # availability can never disguise a missing staged predecessor.
        for dependency in package['dependencies']:
            dep_name = dependency['name']
            if dep_name in self.order and self.order.index(dep_name) < self.current_position:
                if dep_name not in self.staged or not (self.staged[dep_name] / 'Cargo.toml').is_file():
                    raise GateFailure(f'P-MISSING-PREDECESSOR: {name} requires {dep_name}')
        original = tomllib.loads(manifest.read_text())
        if 'workspace' in original or inherited(original):
            if self.policy is None:
                raise GateFailure(f'P-ADMISSION-WORKSPACE: {name}: original descriptor/inheritance '
                                  'requires a separately settled admission boundary')
            self.prepare_workspace(self.policy.context_for(manifest))
        destination = self.destination(manifest)
        destination.mkdir(parents=True, exist_ok=True)
        files = self.inventory(package)
        if len(files) != len(set(files)) or 'Cargo.toml' not in files:
            raise GateFailure(f'P-INCLUDE: invalid inventory for {name}')
        hashes = {}
        origins = {}
        for relative in files:
            if relative in GENERATED:
                continue
            path = Path(relative)
            if path.is_absolute() or '..' in path.parts:
                raise GateFailure(f'P-INCLUDE: unsafe member {name}/{relative}')
            source = manifest.parent / path
            if not source.is_file() and self.policy is not None:
                member = self.policy.members.get(manifest)
                choices = [self.repo / f['path'] for f in member[1]['shared_files']
                           if Path(f['path']).name == path.name] if member else []
                if len(choices) == 1:
                    source = choices[0]
            if not source.is_file():
                raise GateFailure(f'P-INCLUDE: missing real inventory file {source}')
            target = destination / path
            target.parent.mkdir(parents=True, exist_ok=True)
            self.copy_real(source, target)
            hashes[relative] = sha(source)
            origins[relative] = str(source.resolve())
        if (destination / 'Cargo.toml').read_bytes() != manifest.read_bytes():
            raise GateFailure(f'P-ORIGINAL-MANIFEST: {name}')
        self.records[name] = {'manifest': str(manifest), 'source_inventory': files,
                              'payload_sha256': hashes, 'payload_origins': origins,
                              'graph_closure': closure}
        for dependency in package['dependencies']:
            if not dependency.get('path'):
                continue
            dep_manifest = (Path(dependency['path']) / 'Cargo.toml').resolve()
            target_package = self.packages.get(dep_manifest)
            if target_package is None:
                raise GateFailure(f'P-ADMISSION-UNKNOWN: {dep_manifest}')
            target_name = target_package['name']
            alias = self.destination(dep_manifest)
            if self.predecessor(target_package):
                continue
            elif target_name == self.current['name']:
                continue  # A real closure back-edge loads the current source input.
            elif (self.policy is not None and dep_manifest in self.policy.members and
                  self.policy.context_for(dep_manifest)['descriptor'] in self.active_contexts):
                self.add(target_package, closure=True)
            elif closure or (dependency.get('kind') == 'dev' and dependency['req'] == '*'):
                self.add(target_package, closure=True)
            else:
                raise GateFailure(f'P-ORDER: {name} has nonadmitted path dependency {target_name}')

    def preserve_check(self) -> None:
        for source, digest in self.source_files.items():
            if sha(Path(source)) != digest:
                raise GateFailure(f'P-SOURCE-CHANGED: {source}')
        for record in self.records.values():
            for relative, digest in record['payload_sha256'].items():
                source = Path(record['payload_origins'][relative])
                if sha(source) != digest:
                    raise GateFailure(f'P-SOURCE-CHANGED: {source}')


def members(archive: Path) -> dict[str, bytes]:
    result = {}
    with tarfile.open(archive, 'r:gz') as handle:
        for member in handle.getmembers():
            if member.isfile():
                _, relative = member.name.split('/', 1)
                if Path(relative).is_absolute() or '..' in Path(relative).parts or relative in result:
                    raise GateFailure(f'P-INCLUDE: unsafe or duplicate archive member {relative}')
                result[relative] = handle.extractfile(member).read()
    return result


def _execute(args, guards) -> None:
    repo = args.repo.resolve()
    stage = args.stage.resolve()
    documents = [json.loads(args.metadata.read_text()), json.loads(args.ratatui_metadata.read_text())]
    metadata = [p for document in documents for p in document['packages']]
    packages = list({p['manifest_path']: p for p in metadata}.values())
    current = next(p for p in packages if p['name'] == args.crate)
    policy = WorkspacePolicy(repo, json.loads((repo / 'scripts/crates_ci_workspace_contexts.json').read_text()), documents)
    ordered = args.order.read_text().splitlines()
    staged = {}
    for name in ordered[:ordered.index(args.crate)]:
        package = next((p for p in packages if p['name'] == name), None)
        if package is not None:
            directory = stage / 'staged' / f"{name}-{package['version']}"
            if directory.is_dir():
                staged[name] = directory
    receipt_dir = stage / 'inputs' / args.crate
    receipt_dir.mkdir(parents=True, exist_ok=True)
    # Cargo searches ancestor manifests. Inputs under the default repo/target
    # stage would inherit the live workspace even with a separate Git root.
    input_dir = Path(tempfile.mkdtemp(prefix=f'rlvgl-stage-{args.crate}-'))
    graph = input_dir / 'graph'
    head = run(['git', 'rev-parse', 'HEAD'], cwd=repo).stdout.strip()
    source_parent = Path(current['manifest_path']).parent
    vcs_root = Path(run(['git', 'rev-parse', '--show-toplevel'], cwd=source_parent).stdout.strip())
    source_vcs_head = run(['git', 'rev-parse', 'HEAD'], cwd=source_parent).stdout.strip()
    source_vcs_path = source_parent.relative_to(vcs_root).as_posix()
    if source_vcs_path == '.':
        source_vcs_path = ''
    source_vcs_status = run(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=vcs_root).stdout
    lock_paths = [repo / 'Cargo.lock', source_parent / 'Cargo.lock']
    lock_paths += [repo / w['descriptor'].replace('Cargo.toml', 'Cargo.lock') for w in policy.workspaces]
    lock_paths += [repo / m['manifest'].replace('Cargo.toml', 'Cargo.lock')
                   for w in policy.workspaces for m in w['members']]
    source_locks = lock_snapshot(lock_paths)
    run(['git', 'clone', '--shared', '--no-checkout', str(repo), str(graph)], cwd=repo)
    preparation_patch = input_dir / 'preparation-config.toml'
    caller_patch_sha256 = sha(args.patch)
    graph_input = PackageInput(repo, graph, current, packages, ordered, staged,
                               policy=policy, patch=preparation_patch)
    guards.append(graph_input)
    graph_input.refresh_patch()
    graph_input.add(current)
    graph_input.index_payloads()
    manifest = graph_input.destination(Path(current['manifest_path'])) / 'Cargo.toml'
    active = policy.members.get(Path(current['manifest_path']).resolve())
    workspace = active[0] if active and active[0]['descriptor'] in graph_input.active_contexts else None
    seeds = ([repo / workspace['descriptor'].replace('Cargo.toml', 'Cargo.lock')]
             if workspace else [source_parent / 'Cargo.lock', vcs_root / 'Cargo.lock', repo / 'Cargo.lock'])
    seed = next((path for path in seeds if source_locks.get(path) is not None), None)
    if seed is not None and workspace is None:
        shutil.copy2(seed, manifest.parent / 'Cargo.lock')
    lock_seed = source_locks[seed] if seed is not None else None
    target = input_dir / 'target'
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target))
    record = {'qualification': 'detached package diagnostic; full consumer/CI gates separate',
              'source_head': head, 'original_manifest_sha256': sha(Path(current['manifest_path'])),
              'seed_lock_source': str(seed) if seed is not None else None,
              'seed_lock_sha256': lock_seed,
              'source_lock_state': {str(path): digest for path, digest in source_locks.items()},
              'admission': graph_input.records, 'extracted_aliases': graph_input.aliases,
              'workspace_contexts': graph_input.context_records,
              'workspace_context_edges': graph_input.context_edges,
              'preparation_patch': str(preparation_patch),
              'preparation_patch_sha256': sha(preparation_patch),
              'caller_patch_sha256': caller_patch_sha256,
              'source_vcs_root': str(vcs_root), 'source_vcs_head': source_vcs_head,
              'source_vcs_path': source_vcs_path, 'source_vcs_status': source_vcs_status}
    record['input_directory'] = str(input_dir)
    receipt = receipt_dir / 'receipt.json'
    receipt.write_text(json.dumps(record, indent=2) + '\n')
    result = run(['cargo', '--config', str(preparation_patch), 'package', '--no-verify',
                  '--allow-dirty', '--manifest-path', str(manifest)], cwd=graph, env=environment)
    print(result.stdout, end='')
    print(result.stderr, end='')
    archive = target / 'package' / f"{current['name']}-{current['version']}.crate"
    data = members(archive)
    inventory = graph_input.records[current['name']]['source_inventory']
    if set(data) != set(inventory):
        raise GateFailure(f'P-INCLUDE: member difference {sorted(set(data) ^ set(inventory))}')
    if data['Cargo.toml.orig'] != Path(current['manifest_path']).read_bytes():
        raise GateFailure(f"P-ORIGINAL-MANIFEST: {args.crate}")
    vcs = json.loads(data['.cargo_vcs_info.json'])
    vcs_audit(vcs, source_vcs_head, source_vcs_path, bool(source_vcs_status))
    for relative, digest in graph_input.records[current['name']]['payload_sha256'].items():
        if relative != 'Cargo.toml' and hashlib.sha256(data[relative]).hexdigest() != digest:
            raise GateFailure(f'P-PAYLOAD: {relative}')
    extracted = input_dir / 'extracted' / f"{current['name']}-{current['version']}"
    extracted.mkdir(parents=True)
    for relative, content in data.items():
        destination = extracted / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
    forbidden = input_dir / 'admission-retained'
    graph.rename(forbidden)
    allowed = {extracted.resolve() / 'Cargo.toml'} | {p.resolve() / 'Cargo.toml' for p in staged.values()}
    resolved = run(['cargo', '--config', str(args.patch.resolve()), 'metadata', '--locked',
                    '--format-version', '1', '--manifest-path', str(extracted / 'Cargo.toml')],
                   cwd=input_dir, env=environment)
    resolved_metadata = json.loads(resolved.stdout)
    local_graph_audit(resolved_metadata, allowed)
    for package in resolved_metadata['packages']:
        if package['name'] in staged and package.get('source') is not None:
            raise GateFailure(f"P-PREDECESSOR-BYPASS: {package['name']}")
    (receipt_dir / 'extracted-metadata.json').write_text(resolved.stdout)
    if any(sha(extracted / n) != hashlib.sha256(content).hexdigest() for n, content in data.items()):
        raise GateFailure('P-EXTRACTED-MUTATION: metadata changed packaged files')
    graph_input.preserve_check()
    preserve_locks(source_locks)
    if sha(args.patch) != caller_patch_sha256:
        raise GateFailure('P-CALLER-CONFIG: original extracted-only config changed')
    if run(['git', 'rev-parse', 'HEAD'], cwd=repo).stdout.strip() != head:
        raise GateFailure('P-SOURCE-HEAD: frozen candidate changed')
    if run(['git', 'rev-parse', 'HEAD'], cwd=vcs_root).stdout.strip() != source_vcs_head:
        raise GateFailure('P-SOURCE-HEAD: nested source changed')
    graph_lock = (forbidden / workspace['descriptor'].replace('Cargo.toml', 'Cargo.lock')
                  if workspace else forbidden / manifest.relative_to(graph).parent / 'Cargo.lock')
    record.update({'archive_sha256': sha(archive), 'all_member_sha256':
                   {n: hashlib.sha256(content).hexdigest() for n, content in data.items()},
                   'temporary_graph_lock_sha256': sha(graph_lock) if graph_lock.is_file() else None,
                   'metadata_packages': len(json.loads(resolved.stdout)['packages']),
                   'admission_absent_resolution': True})
    receipt.write_text(json.dumps(record, indent=2) + '\n')
    destination = repo / 'target/package' / archive.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(archive, destination)
    print(f'Gate P: bounded detached package input passed for {args.crate}')


def execute(args) -> None:
    """Preserve checkout lock hashes AND absence even on a failed Cargo trial."""
    repo = args.repo.resolve()
    destination = args.stage / 'inputs' / args.crate
    destination.mkdir(parents=True, exist_ok=True)
    for name in ['receipt.json', 'extracted-metadata.json', 'source-preservation.json']:
        (destination / name).unlink(missing_ok=True)
    policy = json.loads((repo / 'scripts/crates_ci_workspace_contexts.json').read_text())
    paths = [repo / 'Cargo.lock']
    paths += [repo / w['descriptor'].replace('Cargo.toml', 'Cargo.lock') for w in policy['workspaces']]
    paths += [repo / m['manifest'].replace('Cargo.toml', 'Cargo.lock')
              for w in policy['workspaces'] for m in w['members']]
    snapshot = lock_snapshot(paths)
    guards = []
    try:
        _execute(args, guards)
    finally:
        for guard in guards:
            guard.preserve_check()
        preserve_locks(snapshot)
        (destination / 'source-preservation.json').write_text(json.dumps({
            'locks': {str(path): digest for path, digest in snapshot.items()},
            'source_payloads_unchanged': True,
            'checkout_lock_hashes_and_absence_preserved': True,
        }, indent=2) + '\n')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['repo', 'stage', 'metadata', 'ratatui-metadata', 'order', 'patch']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--crate', required=True)
    parser.add_argument('--allow-dirty', action='store_true',
                        help='Outer source/preparation gate compatibility; sparse inputs always need it')
    args = parser.parse_args()
    try:
        execute(args)
    except (GateFailure, OSError, ValueError, StopIteration) as error:
        parser.exit(1, f'Gate P: {error}\n')


if __name__ == '__main__':
    main()
