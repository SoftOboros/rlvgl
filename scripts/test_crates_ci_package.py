"""CRATES-CI §6 boundary controls; fixtures do not qualify candidate packages."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import os
import shutil
import subprocess

spec = importlib.util.spec_from_file_location('crates_ci_package', Path(__file__).with_name('crates_ci_package.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PackageBoundary(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / 'source'
        self.repo.mkdir()
        self.graph = self.root / 'admission'
        self.graph.mkdir()
        self.api = self.package('api')
        self.core = self.package('core', [self.dependency('api')])
        self.widgets = self.package('widgets', [self.dependency('core')])
        self.extracted = self.root / 'extracted-api'
        self.extracted.mkdir()
        (self.extracted / 'Cargo.toml').write_text('[package]\nname="api"\nversion="0.0.1"\n')

    def package(self, name, dependencies=None, manifest=None):
        directory = self.repo / name
        (directory / 'src').mkdir(parents=True)
        (directory / 'Cargo.toml').write_text(manifest or f'[package]\nname="{name}"\nversion="0.0.1"\n')
        (directory / 'src/lib.rs').write_text('// Actual fixture inventory payload.\n')
        return {'name': name, 'manifest_path': str(directory / 'Cargo.toml'),
                'dependencies': dependencies or [], 'version': '0.0.1'}

    def dependency(self, name, kind=None, req='*'):
        return {'name': name, 'path': str(self.repo / name), 'kind': kind, 'req': req}

    def input(self, staged=None):
        return module.PackageInput(self.repo, self.graph, self.core,
                                   [self.api, self.core, self.widgets],
                                   ['api', 'core', 'widgets'], staged or {})

    def inventory(self, *_):
        return ['Cargo.toml', 'src/lib.rs', *sorted(module.GENERATED)]

    def test_missing_extracted_predecessor_never_falls_back(self):
        with patch.object(module.PackageInput, 'inventory', self.inventory):
            with self.assertRaisesRegex(module.GateFailure, 'P-MISSING-PREDECESSOR'):
                self.input().add(self.core)
        self.assertFalse((self.graph / 'api').exists())

    def test_registry_spelled_predecessor_missing_before_inventory(self):
        del self.core['dependencies'][0]['path']
        with patch.object(module.PackageInput, 'inventory', side_effect=AssertionError('Cargo must not run')):
            with self.assertRaisesRegex(module.GateFailure, 'P-MISSING-PREDECESSOR'):
                self.input().add(self.core)

    def test_real_shared_payload_origin_is_preserved(self):
        (self.repo / 'README.md').write_text('Real workspace readme.\n')
        policy, metadata = self.workspace_fixture()
        policy['workspaces'][0]['members'][0]['shared_files'] = [
            {'path': 'README.md', 'sha256': module.sha(self.repo / 'README.md')}]
        authority = module.WorkspacePolicy(self.repo, policy, [metadata])
        state = self.input({'api': self.extracted})
        state.policy = authority
        with patch.object(module.PackageInput, 'inventory', return_value=[*self.inventory(), 'README.md']):
            state.add(self.core)
        self.assertEqual(state.records['core']['payload_origins']['README.md'], str((self.repo / 'README.md').resolve()))
        state.preserve_check()
        (self.repo / 'README.md').write_text('Changed workspace readme.\n')
        with self.assertRaisesRegex(module.GateFailure, 'P-SOURCE-CHANGED'):
            state.preserve_check()

    def test_copy_existing_extracted_bytes_never_writes_through_alias(self):
        state = self.input({'api': self.extracted})
        source = self.repo / 'api/Cargo.toml'
        (self.extracted / 'Cargo.toml').write_bytes(source.read_bytes())
        alias = self.graph / 'api'
        alias.symlink_to(self.extracted, target_is_directory=True)
        with patch.object(module.shutil, 'copy2', side_effect=AssertionError('Extracted bytes must not be written')):
            state.copy_real(source, alias / 'Cargo.toml')

    def test_failed_trial_checks_lock_absence(self):
        policy, _ = self.workspace_fixture()
        (self.repo / 'scripts').mkdir()
        import json
        (self.repo / 'scripts/crates_ci_workspace_contexts.json').write_text(json.dumps(policy))
        import argparse
        args = argparse.Namespace(repo=self.repo, stage=self.root / 'failed-stage', crate='core')
        previous = args.stage / 'inputs/core'
        previous.mkdir(parents=True)
        (previous / 'receipt.json').write_text('{"archive_sha256":"stale-success"}')
        (previous / 'extracted-metadata.json').write_text('{"stale":"success"}')
        with patch.object(module, '_execute', side_effect=module.GateFailure('P-CARGO: expected fixture failure')):
            with self.assertRaisesRegex(module.GateFailure, 'expected fixture failure'):
                module.execute(args)
        receipt = json.loads((args.stage / 'inputs/core/source-preservation.json').read_text())
        self.assertTrue(receipt['checkout_lock_hashes_and_absence_preserved'])
        self.assertIsNone(receipt['locks'][str((self.repo / 'Cargo.lock').resolve())])
        self.assertFalse((previous / 'receipt.json').exists())
        self.assertFalse((previous / 'extracted-metadata.json').exists())
        def changes_checkout(_args, _guards):
            (self.repo / 'Cargo.lock').write_text('Unwanted lock.\n')
            raise module.GateFailure('P-CARGO: expected fixture failure')
        with patch.object(module, '_execute', side_effect=changes_checkout):
            with self.assertRaisesRegex(module.GateFailure, 'P-SOURCE-LOCK'):
                module.execute(args)

    def test_predecessor_alias_is_extracted_and_original_manifest_exact(self):
        with patch.object(module.PackageInput, 'inventory', self.inventory):
            state = self.input({'api': self.extracted})
            state.add(self.core)
        self.assertEqual((self.graph / 'api').resolve(), self.extracted.resolve())
        self.assertEqual((self.graph / 'core/Cargo.toml').read_bytes(), (self.repo / 'core/Cargo.toml').read_bytes())

    def test_path_only_dev_closure_and_real_current_back_edge(self):
        self.core['dependencies'].append(self.dependency('widgets', 'dev'))
        with patch.object(module.PackageInput, 'inventory', self.inventory):
            state = self.input({'api': self.extracted})
            state.add(self.core)
        self.assertEqual(set(state.records), {'core', 'widgets'})
        self.assertTrue(state.records['widgets']['graph_closure'])
        self.assertFalse((self.graph / 'core').is_symlink())

    def test_normal_future_dependency_cannot_be_admitted(self):
        self.core['dependencies'].append(self.dependency('widgets', req='0.0.1'))
        with patch.object(module.PackageInput, 'inventory', self.inventory):
            with self.assertRaisesRegex(module.GateFailure, 'P-ORDER'):
                self.input({'api': self.extracted}).add(self.core)

    def test_workspace_inheritance_is_a_stop_not_a_rewrite(self):
        (self.repo / 'core/Cargo.toml').write_text('[package]\nname="core"\nversion.workspace=true\n')
        with patch.object(module.PackageInput, 'inventory', side_effect=AssertionError('Cargo must not run')):
            with self.assertRaisesRegex(module.GateFailure, 'P-ADMISSION-WORKSPACE'):
                self.input({'api': self.extracted}).add(self.core)

    def test_missing_real_inventory_file_fails(self):
        with patch.object(module.PackageInput, 'inventory', return_value=['Cargo.toml', 'missing.rs']):
            with self.assertRaisesRegex(module.GateFailure, 'P-INCLUDE'):
                self.input({'api': self.extracted}).add(self.core)

    def test_source_mutation_is_detected(self):
        with patch.object(module.PackageInput, 'inventory', self.inventory):
            state = self.input({'api': self.extracted})
            state.add(self.core)
        (self.repo / 'core/src/lib.rs').write_text('// Changed after snapshot.\n')
        with self.assertRaisesRegex(module.GateFailure, 'P-SOURCE-CHANGED'):
            state.preserve_check()

    def test_metadata_rejects_source_admission_even_through_alias(self):
        alias = self.root / 'apparently-extracted'
        alias.symlink_to(self.graph, target_is_directory=True)
        metadata = {'packages': [{'name': 'leak', 'source': None,
                                  'manifest_path': str(alias / 'Cargo.toml')}]}
        with self.assertRaisesRegex(module.GateFailure, 'P-ADMISSION-LEAK'):
            module.local_graph_audit(metadata, {self.extracted / 'Cargo.toml'})

    def test_metadata_accepts_extracted_paths_and_registry_only(self):
        package = {'name': 'api', 'source': None, 'manifest_path': str(self.extracted / 'Cargo.toml')}
        module.local_graph_audit({'packages': [package, {'name': 'other', 'source': 'registry+https://example.invalid', 'manifest_path': '/unused'}]}, {self.extracted.resolve() / 'Cargo.toml'})
        package['source'] = 'git+https://example.invalid'
        with self.assertRaisesRegex(module.GateFailure, 'P-RESOLVE-SOURCE'):
            module.local_graph_audit({'packages': [package]}, set())

    def test_absent_checkout_lock_remains_absent_and_existing_lock_immutable(self):
        missing = self.repo / 'Cargo.lock'
        existing = self.repo / 'core/Cargo.lock'
        existing.write_text('# Real fixture lock bytes.\n')
        snapshot = module.lock_snapshot([missing, existing])
        self.assertIsNone(snapshot[missing])
        module.preserve_locks(snapshot)
        missing.write_text('# Unwanted source-side generation.\n')
        with self.assertRaisesRegex(module.GateFailure, 'P-SOURCE-LOCK'):
            module.preserve_locks(snapshot)
        missing.unlink()
        existing.write_text('# Changed source-side lock.\n')
        with self.assertRaisesRegex(module.GateFailure, 'P-SOURCE-LOCK'):
            module.preserve_locks(snapshot)

    def workspace_fixture(self):
        descriptor = self.repo / 'Cargo.toml'
        descriptor.write_text('[workspace]\nmembers=["core"]\n')
        p = dict(self.core, id='core-fixture', targets=[{'src_path': str(self.repo / 'core/src/lib.rs')}])
        member = {'name': 'core', 'manifest': 'core/Cargo.toml',
                  'manifest_sha256': module.sha(self.repo / 'core/Cargo.toml'),
                  'targets': ['core/src/lib.rs'], 'shared_files': []}
        context = {'descriptor': 'Cargo.toml', 'descriptor_sha256': module.sha(descriptor),
                   'members': [member]}
        metadata = {'workspace_root': str(self.repo), 'packages': [p],
                    'workspace_members': ['core-fixture']}
        return {'workspaces': [context]}, metadata

    def test_real_descriptor_member_target_context_accepts_exact_bytes(self):
        policy, metadata = self.workspace_fixture()
        before = (self.repo / 'core/Cargo.toml').read_bytes()
        state = module.WorkspacePolicy(self.repo, policy, [metadata])
        self.assertEqual(state.context_for(self.repo / 'core/Cargo.toml'), policy['workspaces'][0])
        self.assertEqual((self.repo / 'core/Cargo.toml').read_bytes(), before)

    def test_changed_real_descriptor_refused(self):
        policy, metadata = self.workspace_fixture()
        (self.repo / 'Cargo.toml').write_text('[workspace]\nmembers=["core", "widgets"]\n')
        with self.assertRaisesRegex(module.GateFailure, 'P-DESCRIPTOR-CHANGED'):
            module.WorkspacePolicy(self.repo, policy, [metadata])

    def test_extra_workspace_member_refused(self):
        policy, metadata = self.workspace_fixture()
        metadata['packages'].append(dict(self.widgets, id='widgets-fixture'))
        metadata['workspace_members'].append('widgets-fixture')
        with self.assertRaisesRegex(module.GateFailure, 'P-WORKSPACE-MEMBERS'):
            module.WorkspacePolicy(self.repo, policy, [metadata])

    def test_extra_member_target_refused(self):
        policy, metadata = self.workspace_fixture()
        metadata['packages'][0]['targets'].append({'src_path': str(self.repo / 'core/another.rs')})
        with self.assertRaisesRegex(module.GateFailure, 'P-WORKSPACE-TARGETS'):
            module.WorkspacePolicy(self.repo, policy, [metadata])

    def test_unenumerated_inherited_workspace_refused(self):
        policy, metadata = self.workspace_fixture()
        state = module.WorkspacePolicy(self.repo, policy, [metadata])
        with self.assertRaisesRegex(module.GateFailure, 'P-ADMISSION-WORKSPACE'):
            state.context_for(self.repo / 'widgets/Cargo.toml')

    def test_nested_vcs_rejects_outer_head_path_and_hidden_dirty_overlay(self):
        good = {'git': {'sha1': 'nested-ed', 'dirty': True}, 'path_in_vcs': 'ratatui-rlvgl'}
        module.vcs_audit(good, 'nested-ed', 'ratatui-rlvgl', True)
        with self.assertRaisesRegex(module.GateFailure, 'P-VCS'):
            module.vcs_audit(good, 'outer-aa', 'vendor/ratatui/ratatui-rlvgl', True)
        good['git']['dirty'] = False
        with self.assertRaisesRegex(module.GateFailure, 'P-VCS-DIRTY'):
            module.vcs_audit(good, 'nested-ed', 'ratatui-rlvgl', True)

    def test_untracked_member_payload_is_refused_including_ignored_selection(self):
        manifests = [self.repo / 'core/Cargo.toml']
        module.untracked_member_audit(self.repo, ['core/Cargo.lock', 'unselected.md'], manifests)
        for relative in ['core/src/new.rs', 'core/generated-explicit-include.bin']:
            with self.assertRaisesRegex(module.GateFailure, 'P-WORKSPACE-UNTRACKED'):
                module.untracked_member_audit(self.repo, [relative], manifests)

    def test_real_cargo_tracked_ignored_inventory_and_archive_are_preserved(self):
        assets = self.repo / 'api/assets'
        assets.mkdir()
        (assets / '.gitignore').write_text('*.bin\n')
        (assets / 'required.bin').write_bytes(b'Actual selected ignored fixture payload.\n')
        environment = dict(os.environ, CARGO_NET_OFFLINE='true', CARGO_TARGET_DIR=str(self.root / 'cargo-target'))
        def command(args, cwd):
            return module.run(args, cwd=cwd, env=environment)
        command(['git', 'init'], self.repo)
        command(['git', 'add', '-f', '--', 'api'], self.repo)
        command(['git', '-c', 'user.name=Boundary Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-m', 'Disposable tracked inventory fixture'], self.repo)
        shutil.rmtree(self.graph)
        command(['git', 'clone', '--shared', '--no-checkout', str(self.repo), str(self.graph)], self.root)
        state = module.PackageInput(self.repo, self.graph, self.api, [self.api], ['api'], {})
        state.add(self.api)
        manifest = self.graph / 'api/Cargo.toml'
        inventory_command = ['cargo', 'package', '--list', '--allow-dirty', '--manifest-path', str(manifest)]
        before = set(command(inventory_command, self.graph).stdout.splitlines())
        expected = set(state.records['api']['source_inventory'])
        self.assertIn('assets/.gitignore', expected)
        self.assertIn('assets/required.bin', expected)
        self.assertNotEqual(before, expected)
        source_index = command(['git', 'ls-files', '--stage'], self.repo).stdout
        state.index_payloads()
        self.assertEqual(set(command(inventory_command, self.graph).stdout.splitlines()), expected)
        command(['cargo', 'package', '--no-verify', '--allow-dirty', '--manifest-path', str(manifest)], self.graph)
        archive = self.root / 'cargo-target/package/api-0.0.1.crate'
        self.assertEqual(set(module.members(archive)), expected)
        self.assertEqual(module.members(archive)['assets/required.bin'], (assets / 'required.bin').read_bytes())
        self.assertEqual(command(['git', 'ls-files', '--stage'], self.repo).stdout, source_index)

    def test_real_mixed_path_registry_alias_identity_and_withdrawal(self):
        core_manifest = self.repo / 'core/Cargo.toml'
        core_manifest.write_text(core_manifest.read_text() + '[dependencies]\napi="0.0.1"\n')
        widgets_manifest = self.repo / 'widgets/Cargo.toml'
        widgets_manifest.write_text(widgets_manifest.read_text() +
            '[dependencies]\napi={path="../api",version="0.0.1"}\ncore={path="../core",version="0.0.1"}\n')
        self.widgets['dependencies'].append(self.dependency('api', req='0.0.1'))
        environment = dict(os.environ, CARGO_NET_OFFLINE='true', CARGO_TARGET_DIR=str(self.root / 'cargo-target'))
        def command(args, cwd):
            return module.run(args, cwd=cwd, env=environment)
        command(['git', 'init'], self.repo)
        command(['git', 'add', '-f', '--', 'api', 'core', 'widgets'], self.repo)
        command(['git', '-c', 'user.name=Boundary Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-m', 'Disposable mixed-route fixture'], self.repo)
        extracted_temp = tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve())
        self.addCleanup(extracted_temp.cleanup)
        staged_root = Path(extracted_temp.name)
        staged = {}; caller = self.root / 'caller-config.toml'
        def caller_config():
            caller.write_text('[patch.crates-io]\n' + ''.join(
                f'{name}={{path="{path}"}}\n' for name, path in staged.items()))
        for name in ['api', 'core']:
            caller_config()
            command(['cargo', '--config', str(caller), 'package', '--no-verify', '--allow-dirty',
                     '--manifest-path', str(self.repo / name / 'Cargo.toml')], self.repo)
            archive = self.root / f'cargo-target/package/{name}-0.0.1.crate'
            target = staged_root / f'{name}-0.0.1'; target.mkdir(parents=True)
            for relative, content in module.members(archive).items():
                path = target / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(content)
            staged[name] = target
        caller_config(); original_config = caller.read_bytes()
        shutil.rmtree(self.graph)
        command(['git', 'clone', '--shared', '--no-checkout', str(self.repo), str(self.graph)], self.root)
        private = self.root / 'preparation-config.toml'
        state = module.PackageInput(self.repo, self.graph, self.widgets,
                                    [self.api, self.core, self.widgets], ['api', 'core', 'widgets'], staged,
                                    patch=private)
        with patch.object(module.PackageInput, 'inventory', self.inventory):
            state.add(self.widgets)
        state.index_payloads()
        manifest = self.graph / 'widgets/Cargo.toml'
        args = ['package', '--no-verify', '--allow-dirty', '--manifest-path', str(manifest)]
        with self.assertRaisesRegex(module.GateFailure, 'package collision in the lockfile'):
            command(['cargo', '--config', str(caller), 'metadata', '--format-version', '1',
                     '--manifest-path', str(manifest)], self.graph)
        command(['cargo', '--config', str(private), *args], self.graph)
        archive = self.root / 'cargo-target/package/widgets-0.0.1.crate'
        extracted = self.root / 'packaged-widgets'; extracted.mkdir()
        for relative, content in module.members(archive).items():
            path = extracted / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(content)
        self.graph.rename(self.root / 'admission-withdrawn')
        resolved = command(['cargo', '--config', str(caller), 'metadata', '--locked', '--format-version', '1',
                            '--manifest-path', str(extracted / 'Cargo.toml')], self.root)
        import json
        metadata = json.loads(resolved.stdout)
        allowed = {extracted.resolve() / 'Cargo.toml'} | {p.resolve() / 'Cargo.toml' for p in staged.values()}
        module.local_graph_audit(metadata, allowed)
        self.assertEqual(len(metadata['packages']), 3)
        self.assertEqual(caller.read_bytes(), original_config)

    def git_fixture(self, directory, paths):
        module.run(['git', 'init'], cwd=directory)
        module.run(['git', 'add', '-f', '--', *paths], cwd=directory)
        module.run(['git', '-c', 'user.name=Boundary Fixture', '-c', 'user.email=fixture@example.invalid',
                    'commit', '-m', 'Disposable finite context fixture'], cwd=directory)

    def test_genuine_nested_identity_preserves_preexisting_extracted_bridge_alias(self):
        nested = self.repo / 'vendor/ratatui'; nested.mkdir(parents=True)
        (nested / 'README.md').write_text('Actual nested fixture.\n')
        self.git_fixture(nested, ['README.md'])
        bridge = dict(self.api, name='bridge', manifest_path=str(nested / 'ratatui-rlvgl/Cargo.toml'))
        state = module.PackageInput(self.repo, self.graph, self.core, [self.core, bridge], ['bridge', 'core'],
                                    {'bridge': self.extracted})
        self.assertTrue(state.predecessor(bridge))
        before = (self.extracted / 'Cargo.toml').read_bytes()
        state.attach_nested_git(nested)
        projected = self.graph / 'vendor/ratatui'
        self.assertEqual(module.run(['git', 'rev-parse', 'HEAD'], cwd=projected).stdout,
                         module.run(['git', 'rev-parse', 'HEAD'], cwd=nested).stdout)
        self.assertEqual((projected / 'ratatui-rlvgl').resolve(), self.extracted.resolve())
        self.assertEqual((self.extracted / 'Cargo.toml').read_bytes(), before)

    def test_finite_cross_context_cycle_and_outside_union_refusal(self):
        nested = self.repo / 'vendor/ratatui'; nested.mkdir(parents=True)
        (nested / 'Cargo.toml').write_text('[workspace]\nmembers=["widgets"]\n')
        (self.repo / 'Cargo.toml').write_text('[workspace]\nmembers=["core"]\n')
        directory = nested / 'widgets'; (directory / 'src').mkdir(parents=True)
        (directory / 'Cargo.toml').write_text('[package]\nname="widgets"\nversion="0.0.1"\n')
        (directory / 'src/lib.rs').write_text('// Actual nested target.\n')
        widget = dict(self.widgets, manifest_path=str(directory / 'Cargo.toml'),
                      dependencies=[self.dependency('core')])
        core = dict(self.core, dependencies=[{'name':'widgets', 'path':str(directory), 'kind':None, 'req':'*'}])
        contexts=[]; documents=[]
        for descriptor, package in [('Cargo.toml',core),('vendor/ratatui/Cargo.toml',widget)]:
            manifest=Path(package['manifest_path']); relative=manifest.relative_to(self.repo)
            member={'name':package['name'],'manifest':str(relative),'manifest_sha256':module.sha(manifest),
                    'targets':[str(relative.parent/'src/lib.rs')],'shared_files':[]}
            contexts.append({'descriptor':descriptor,'descriptor_sha256':module.sha(self.repo/descriptor),'members':[member]})
            documents.append({'workspace_root':str((self.repo/descriptor).parent),
                              'workspace_members':[package['name']], 'packages':[dict(package,id=package['name'],
                              targets=[{'src_path':str(manifest.parent/'src/lib.rs')}])]})
        policy=module.WorkspacePolicy(self.repo,{'workspaces':contexts},documents)
        self.git_fixture(nested,['Cargo.toml','widgets'])
        self.git_fixture(self.repo,['Cargo.toml','core','vendor/ratatui'])
        shutil.rmtree(self.graph)
        module.run(['git','clone','--shared','--no-checkout',str(self.repo),str(self.graph)],cwd=self.root)
        actual_run=module.run
        def controlled(command, **kwargs):
            if command[0]=='cargo':
                return subprocess.CompletedProcess(command,0,'\n'.join(self.inventory())+'\n','')
            return actual_run(command,**kwargs)
        state=module.PackageInput(self.repo,self.graph,core,[core,widget,self.api],['core'],{},policy=policy,
                                  patch=self.root/'prep-config.toml')
        state.refresh_patch()
        with patch.object(module,'run',side_effect=controlled):
            state.prepare_workspace(contexts[0])
        self.assertEqual(state.active_contexts,{'Cargo.toml','vendor/ratatui/Cargo.toml'})
        self.assertEqual(len(state.context_edges),2)
        self.assertEqual((self.graph/'vendor/ratatui/widgets/Cargo.toml').read_bytes(),(directory/'Cargo.toml').read_bytes())
        core['dependencies'].append(self.dependency('api'))
        state2=module.PackageInput(self.repo,self.root/'other-graph',core,[core,widget,self.api],['core'],{},policy=policy,
                                   patch=self.root/'other-prep.toml')
        module.run(['git','clone','--shared','--no-checkout',str(self.repo),str(state2.graph)],cwd=self.root)
        state2.refresh_patch()
        with patch.object(module,'run',side_effect=controlled):
            with self.assertRaisesRegex(module.GateFailure,'P-WORKSPACE-CLOSURE'):
                state2.prepare_workspace(contexts[0])


    def owned_bridge_context_fixture(self):
        """Root-owned bridge and official native core have distinct real descriptors."""
        bridge_dir = self.repo / 'crates/ratatui-rlvgl'
        native_dir = self.repo / 'vendor/ratatui/ratatui-core'
        packages = []
        for name, directory in [('ratatui-rlvgl', bridge_dir), ('ratatui-core', native_dir)]:
            (directory / 'src').mkdir(parents=True)
            (directory / 'Cargo.toml').write_text(f'[package]\nname="{name}"\nversion="0.1.2"\n')
            (directory / 'src/lib.rs').write_text('// Actual finite carrier fixture source.\n')
            packages.append({'name': name, 'id': name, 'manifest_path': str(directory / 'Cargo.toml'),
                             'targets': [{'src_path': str(directory / 'src/lib.rs')}]})
        descriptors = [('Cargo.toml', '[workspace]\nmembers=["crates/ratatui-rlvgl"]\n'),
                       ('vendor/ratatui/Cargo.toml', '[workspace]\nmembers=["ratatui-core"]\n')]
        contexts, documents = [], []
        for (relative, content), package in zip(descriptors, packages):
            descriptor = self.repo / relative
            descriptor.write_text(content)
            manifest = Path(package['manifest_path'])
            member = {'name': package['name'], 'manifest': str(manifest.relative_to(self.repo)),
                      'manifest_sha256': module.sha(manifest),
                      'targets': [str((manifest.parent / 'src/lib.rs').relative_to(self.repo))],
                      'shared_files': []}
            contexts.append({'descriptor': relative, 'descriptor_sha256': module.sha(descriptor),
                             'members': [member]})
            documents.append({'workspace_root': str(descriptor.parent),
                              'workspace_members': [package['id']], 'packages': [package]})
        return {'workspaces': contexts}, documents, packages

    def test_owned_bridge_and_native_core_preserve_separate_finite_authority(self):
        policy, documents, packages = self.owned_bridge_context_fixture()
        manifests = [Path(package['manifest_path']) for package in packages]
        before = {str(path): path.read_bytes() for path in manifests}
        authority = module.WorkspacePolicy(self.repo, policy, documents)
        self.assertEqual(authority.context_for(manifests[0])['descriptor'], 'Cargo.toml')
        self.assertEqual(authority.context_for(manifests[1])['descriptor'], 'vendor/ratatui/Cargo.toml')
        self.assertEqual({str(path): path.read_bytes() for path in manifests}, before)
        legacy = self.repo / 'vendor/ratatui/ratatui-rlvgl/Cargo.toml'
        legacy.parent.mkdir()
        legacy.write_bytes(manifests[0].read_bytes())
        with self.assertRaisesRegex(module.GateFailure, 'P-ADMISSION-WORKSPACE'):
            authority.context_for(legacy)

    def test_owned_bridge_does_not_admit_extra_native_member(self):
        policy, documents, packages = self.owned_bridge_context_fixture()
        extra = dict(packages[1], id='unexpected-native', name='unexpected-native',
                     manifest_path=str(self.repo / 'vendor/ratatui/unexpected/Cargo.toml'))
        documents[1]['packages'].append(extra)
        documents[1]['workspace_members'].append(extra['id'])
        with self.assertRaisesRegex(module.GateFailure, 'P-WORKSPACE-MEMBERS'):
            module.WorkspacePolicy(self.repo, policy, documents)

    def test_owned_bridge_does_not_admit_extra_target(self):
        policy, documents, packages = self.owned_bridge_context_fixture()
        packages[0]['targets'].append({'src_path': str(self.repo / 'crates/ratatui-rlvgl/src/unexpected.rs')})
        with self.assertRaisesRegex(module.GateFailure, 'P-WORKSPACE-TARGETS'):
            module.WorkspacePolicy(self.repo, policy, documents)


if __name__ == '__main__':
    unittest.main(verbosity=2)
