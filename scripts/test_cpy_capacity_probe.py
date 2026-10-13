#!/usr/bin/env python3
"""Focused tests for CPY-03 capacity evidence generation and validation."""

from __future__ import annotations

import copy
import importlib.util
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name("cpy_capacity_probe.py")
SPEC = importlib.util.spec_from_file_location("cpy_capacity_probe", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
cpy_capacity_probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cpy_capacity_probe)


def _distribution(samples: int) -> dict[str, int]:
    """Return one monotonic synthetic latency distribution."""

    return {
        "samples": samples,
        "minimum_ns": 10,
        "p50_ns": 20,
        "p95_ns": 30,
        "p99_ns": 40,
        "maximum_ns": 50,
        "mean_ns": 25,
    }


def _probe(scenario: str) -> dict[str, object]:
    """Return one internally consistent synthetic native-probe result."""

    accepted = 8 if scenario == "cold-burst" else 32
    return {
        "schema_version": "CPY-CAPACITY-PROBE-1",
        "workload": "bounded-crossbeam-transport-with-empty-endpoint-safe-turn",
        "retained_bytes_scope": "synthetic fixture",
        "config": {
            "scenario": scenario,
            "ingress_capacity": 8,
            "egress_capacity": 16,
            "turn_budget": 4,
            "messages": 32,
            "ingress_payload_bytes": 256,
            "egress_payload_bytes": 128,
            "observer_stall_us": 50_000 if scenario == "observer-stall" else 0,
            "retry_backoff_us": 50,
            "sampling_hold_us": 20_000,
        },
        "offered_requests": 32,
        "accepted_requests": accepted,
        "terminal_admission_rejections": 24 if scenario == "cold-burst" else 0,
        "ingress_full_observations": 24 if scenario == "cold-burst" else 1,
        "completed_records": accepted,
        "sequence_errors": 0,
        "service_turns": 2 if scenario == "cold-burst" else 8,
        "ingress_empty_to_nonempty_observations": 1,
        "egress_empty_to_nonempty_observations": 1,
        "egress_backpressured_records": 1 if scenario == "observer-stall" else 0,
        "egress_backpressure_ns": 100 if scenario == "observer-stall" else 0,
        "peak_ingress_depth": 8,
        "peak_egress_depth": 8,
        "peak_owned_envelope_bytes": 4096,
        "owned_envelope_bytes_at_end": 0,
        "service_latency": _distribution(accepted),
        "delivery_latency": _distribution(accepted),
        "probe_duration_ns": 1000,
        "service_checksum": 1,
        "consumer_checksum": 2,
    }


def _bundle() -> dict[str, object]:
    """Return a complete one-candidate, three-iteration evidence fixture."""

    runs = [
        {
            "candidate_id": "i8-e16-t4",
            "iteration": iteration,
            "peak_rss_bytes": 1_000_000,
            "rss_source": "darwin-ps-rss",
            "probe": _probe(scenario),
        }
        for scenario in cpy_capacity_probe.SCENARIOS
        for iteration in range(1, 4)
    ]
    summaries = [
        {
            "candidate_id": "i8-e16-t4",
            "scenario": scenario,
            "samples": 3,
            "median_peak_rss_bytes": 1_000_000,
            "median_probe_duration_ns": 1000,
            "median_peak_owned_envelope_bytes": 4096,
            "median_service_p95_ns": 30,
            "median_delivery_p95_ns": 30,
            "maximum_ingress_full_observations": 24,
            "maximum_egress_backpressured_records": 1,
            "all_sequence_clean": True,
            "all_envelope_bytes_released": True,
        }
        for scenario in cpy_capacity_probe.SCENARIOS
    ]
    source_record = {"path": "fixture", "sha256": f"sha256:{'0' * 64}"}
    return {
        "schema_version": "CPY-CAPACITY-EVIDENCE-1",
        "evidence_id": "CPY-CAPACITY-FIXTURE",
        "created_at": "2026-08-18T00:00:00+00:00",
        "qualification": "diagnostic-host",
        "profile": "host-headless",
        "normative_decision": False,
        "source": {
            "commit": "0" * 40,
            "tree_clean": True,
            "probe_sources": [copy.deepcopy(source_record) for _ in range(3)],
            "cargo_lock": copy.deepcopy(source_record),
        },
        "toolchain": {
            "rustc_version": "rustc fixture",
            "cargo_version": "cargo fixture",
            "python_version": "3.13.0",
            "crossbeam_channel": {
                "version": "0.5.16",
                "checksum": f"sha256:{'1' * 64}",
            },
            "probe_binary_sha256": f"sha256:{'2' * 64}",
            "build_profile": "release",
        },
        "environment": {
            "hardware_label": "fixture-host",
            "operating_system": "fixture-os",
            "kernel": "fixture-kernel",
            "machine": "x86_64",
            "cpu_model": "fixture-cpu",
            "rust_host_triple": "x86_64-apple-darwin",
            "logical_cpus": 4,
            "total_memory_bytes": 1_000_000,
            "rss_sampler": "darwin-ps-rss",
            "physical_board": False,
        },
        "method": {
            "clock": "std::time::Instant monotonic clock",
            "warmups": 1,
            "iterations": 3,
            "messages": 32,
            "ingress_payload_bytes": 256,
            "egress_payload_bytes": 128,
            "observer_stall_us": 50_000,
            "retry_backoff_us": 50,
            "sampling_hold_us": 20_000,
            "scenarios": list(cpy_capacity_probe.SCENARIOS),
            "candidates": [
                {
                    "candidate_id": "i8-e16-t4",
                    "ingress_capacity": 8,
                    "egress_capacity": 16,
                    "turn_budget": 4,
                }
            ],
            "limitations": [f"fixture limitation {index}" for index in range(5)],
        },
        "runs": runs,
        "summaries": summaries,
    }


class ParserTests(unittest.TestCase):
    """Exercise exact candidate tuple parsing."""

    def test_candidate_triplet(self) -> None:
        """Positive ingress, egress, and turn values are preserved."""

        self.assertEqual(cpy_capacity_probe._parse_candidate("8:16:4"), (8, 16, 4))

    def test_zero_candidate_is_rejected(self) -> None:
        """Zero cannot become a queue or per-turn candidate."""

        with self.assertRaises(Exception):
            cpy_capacity_probe._parse_candidate("8:0:4")

    def test_representative_manifest_binds_direct_neutral_sources(self) -> None:
        """The v3 manifest covers every direct Stage/input/Cue/render component."""

        paths = cpy_capacity_probe.REPRESENTATIVE_SOURCE_PATHS
        self.assertEqual(len(paths), len(set(paths)))
        self.assertTrue(
            {
                "runtime-std/tests/native_service.rs",
                "core/src/direction.rs",
                "core/src/endpoint.rs",
                "core/src/event.rs",
                "core/src/object.rs",
                "core/src/subscription.rs",
                "widgets/src/container.rs",
                "widgets/src/slider.rs",
            }.issubset(paths)
        )


class EvidenceTests(unittest.TestCase):
    """Exercise the full matrix and deliberate accounting failures."""

    def test_complete_bundle_passes(self) -> None:
        """The fixture satisfies both formal and semantic validation."""

        bundle = _bundle()
        cpy_capacity_probe._validate_schema(bundle)
        cpy_capacity_probe.validate_bundle_value(bundle)

    def test_native_service_probe_version_passes(self) -> None:
        """The production-service workload is distinct from retained v1 evidence."""

        bundle = _bundle()
        for run in bundle["runs"]:
            run["probe"]["schema_version"] = "CPY-CAPACITY-PROBE-2"
            run["probe"]["workload"] = (
                "bounded-native-service-with-empty-endpoint-safe-turn-and-os-readiness"
            )
        cpy_capacity_probe._validate_schema(bundle)
        cpy_capacity_probe.validate_bundle_value(bundle)

    def test_legacy_probe_cannot_claim_representative_fields(self) -> None:
        """A v1/v2 label cannot carry fields reserved for the v3 workload."""

        bundle = _bundle()
        bundle["runs"][0]["probe"]["frame_private"] = True
        with self.assertRaises(cpy_capacity_probe.EvidenceError):
            cpy_capacity_probe.validate_bundle_value(bundle)

    def test_representative_probe_version_passes(self) -> None:
        """The v3 workload proves neutral semantics, cadence, and private rendering."""

        bundle = _bundle()
        for run in bundle["runs"]:
            probe = run["probe"]
            probe["schema_version"] = "CPY-CAPACITY-PROBE-3"
            probe["workload"] = (
                "bounded-native-service-with-stage-input-cues-private-rgba-and-os-readiness"
            )
            probe["config"]["frame_period_us"] = 16_667
            workload_requests = probe["accepted_requests"] + (
                1 if probe["config"]["scenario"] == "cold-burst" else 0
            )
            probe.update(
                {
                    "workload_requests": workload_requests,
                    "stage_batches": probe["service_turns"],
                    "stage_completions": probe["service_turns"],
                    "native_inputs": workload_requests,
                    "cue_records": workload_requests,
                    "frames_rendered": probe["service_turns"],
                    "cadence_misses": 0,
                    "final_stage_revision": 10,
                    "frame_checksum": 1,
                    "frame_width": 320,
                    "frame_height": 240,
                    "frame_stride": 1280,
                    "frame_format": "RGBA8888",
                    "frame_private": True,
                }
            )
        for summary in bundle["summaries"]:
            summary.update(
                {
                    "median_frames_rendered": 8,
                    "median_cadence_misses": 0,
                    "all_representative_semantics_exact": True,
                }
            )
        cpy_capacity_probe._validate_schema(bundle)
        cpy_capacity_probe.validate_bundle_value(bundle)

    def test_representative_probe_rejects_missing_cue(self) -> None:
        """One input without its neutral Cue invalidates representative evidence."""

        bundle = _bundle()
        for run in bundle["runs"]:
            probe = run["probe"]
            probe["schema_version"] = "CPY-CAPACITY-PROBE-3"
            probe["workload"] = (
                "bounded-native-service-with-stage-input-cues-private-rgba-and-os-readiness"
            )
            probe["config"]["frame_period_us"] = 16_667
            workload_requests = probe["accepted_requests"] + (
                1 if probe["config"]["scenario"] == "cold-burst" else 0
            )
            probe.update(
                {
                    "workload_requests": workload_requests,
                    "stage_batches": probe["service_turns"],
                    "stage_completions": probe["service_turns"],
                    "native_inputs": workload_requests,
                    "cue_records": workload_requests,
                    "frames_rendered": probe["service_turns"],
                    "cadence_misses": 0,
                    "final_stage_revision": 10,
                    "frame_checksum": 1,
                    "frame_width": 320,
                    "frame_height": 240,
                    "frame_stride": 1280,
                    "frame_format": "RGBA8888",
                    "frame_private": True,
                }
            )
        for summary in bundle["summaries"]:
            summary.update(
                {
                    "median_frames_rendered": 8,
                    "median_cadence_misses": 0,
                    "all_representative_semantics_exact": True,
                }
            )
        bundle["runs"][0]["probe"]["cue_records"] -= 1
        with self.assertRaises(cpy_capacity_probe.EvidenceError):
            cpy_capacity_probe.validate_bundle_value(bundle)

    def test_missing_iteration_is_rejected(self) -> None:
        """A partial candidate/scenario matrix cannot be summarized as evidence."""

        bundle = _bundle()
        bundle["runs"].pop()
        with self.assertRaises(cpy_capacity_probe.EvidenceError):
            cpy_capacity_probe.validate_bundle_value(bundle)

    def test_sequence_drift_is_rejected(self) -> None:
        """One out-of-order record invalidates the native run."""

        bundle = _bundle()
        bundle["runs"][0]["probe"]["sequence_errors"] = 1
        with self.assertRaises(cpy_capacity_probe.EvidenceError):
            cpy_capacity_probe.validate_bundle_value(bundle)

    def test_capacity_overrun_is_rejected(self) -> None:
        """Observed queue depth may never exceed the configured bound."""

        bundle = _bundle()
        bundle["runs"][0]["probe"]["peak_ingress_depth"] = 9
        with self.assertRaises(cpy_capacity_probe.EvidenceError):
            cpy_capacity_probe.validate_bundle_value(bundle)


if __name__ == "__main__":
    unittest.main(verbosity=2)
