#!/usr/bin/env python3
"""Translate en.json to target locales using OpenAI GPT-5-nano.

Adapted from softoboros/scripts/claude_translate_i18n_json.py for rlvgl's
flat 23-key i18n JSON files.

Usage:
    python3 translate_locale.py --locale de              # translate to German
    python3 translate_locale.py --locale all              # all target locales (skip existing)
    python3 translate_locale.py --locale all --force      # regenerate all
    python3 translate_locale.py --locale de --dry-run     # preview only

Requires:
    pip install openai
    OPENAI_API_KEY environment variable
"""

import argparse
import json
import os
import re
import sys
import time
from pathlib import Path

import openai

LOCALE_TO_LANGUAGE = {
    "fr": "French",
}

DEFAULT_MODEL = os.environ.get("OPENAI_TRANSLATE_MODEL", "gpt-5-nano-2025-08-07")
TAG = "[rlvgl-i18n]"
PLACEHOLDER_RE = re.compile(r"\{[a-z_]+\}")


def build_messages(source_json, target_lang, locale):
    """Build system/user message pair for translation prompt.

    Keeps the long system prefix stable for prompt-cache reuse across locales.
    """
    json_content = json.dumps(source_json, indent=2, ensure_ascii=False)
    system_content = (
        "You are a professional translator for embedded device UI text. "
        "Your task is to translate JSON values ONLY.\n\n"
        "CRITICAL INSTRUCTIONS:\n"
        "1. Translate ONLY the string VALUES\n"
        "2. Keep all JSON keys UNCHANGED — byte-for-byte identical to input\n"
        "3. Keep all structure exactly: brackets {}, colons :, commas ,\n"
        "4. Preserve placeholders like {count}, {name}, {version}, {x}, {y} — DO NOT TRANSLATE\n"
        "5. Return ONLY raw JSON output — NO markdown code blocks, NO explanation text\n"
        "6. Do NOT wrap output in ```json or ``` — output plain JSON only\n"
        "7. Keep translations concise — these are short UI strings for a small embedded display\n"
        "---\n"
        f"{json_content}\n"
        "---"
    )
    user_content = (
        f"Translate all string values to {target_lang} ({locale}). "
        "Return only valid JSON."
    )
    return system_content, user_content


def call_api(client, model, source_json, target_lang, locale):
    """Call OpenAI and return parsed translated dict."""
    system_content, user_content = build_messages(source_json, target_lang, locale)
    response = client.chat.completions.create(
        model=model,
        messages=[
            {"role": "system", "content": system_content},
            {"role": "user", "content": user_content},
        ],
    )
    text = response.choices[0].message.content.strip()
    if not text:
        raise ValueError("Empty response from API")

    # Direct parse
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        pass

    # Markdown fence extraction
    m = re.search(r"```(?:json)?\s*\n([\s\S]*?)\n```", text, re.DOTALL)
    if m:
        return json.loads(m.group(1))

    # Brace extraction
    first, last = text.find("{"), text.rfind("}")
    if first != -1 and last > first:
        return json.loads(text[first : last + 1])

    raise ValueError(f"Could not parse JSON from response: {text[:200]}")


def remap_keys(source_json, translated):
    """Map translated output back onto exact source keys with normalised fallback."""
    norm_lookup = {k.lower().strip(): v for k, v in translated.items()}
    out, remapped, missing = {}, [], []
    for src_key in source_json:
        if src_key in translated:
            out[src_key] = translated[src_key]
        elif src_key.lower().strip() in norm_lookup:
            out[src_key] = norm_lookup[src_key.lower().strip()]
            remapped.append(src_key)
        else:
            out[src_key] = source_json[src_key]
            missing.append(src_key)
    return out, remapped, missing


def validate_placeholders(source, translated):
    """Check that {placeholder} tokens in each value match between source and translation."""
    errors = []
    for key in source:
        src_ph = set(PLACEHOLDER_RE.findall(source[key]))
        tgt_ph = set(PLACEHOLDER_RE.findall(translated.get(key, "")))
        if src_ph != tgt_ph:
            errors.append(f"  {key}: expected {src_ph}, got {tgt_ph}")
    return errors


def translate_locale(
    source_path,
    locale,
    outdir,
    dry_run=False,
    force=False,
    max_retries=3,
    model=DEFAULT_MODEL,
):
    """Translate source JSON to a single target locale."""
    target_lang = LOCALE_TO_LANGUAGE.get(locale)
    if target_lang is None:
        print(f"{TAG} Unknown locale: {locale}")
        print(f"{TAG} Supported: {', '.join(sorted(LOCALE_TO_LANGUAGE))}")
        return False

    outfile = Path(outdir) / f"{locale}.json"

    print(f"{TAG} Source:  {source_path}")
    print(f"{TAG} Locale:  {locale} ({target_lang})")
    print(f"{TAG} Output:  {outfile}")
    print(f"{TAG} Model:   {model}")

    if outfile.exists() and not force:
        print(f"{TAG} Skipping {locale}: {outfile} already exists (use --force to overwrite)")
        return True

    if dry_run:
        print(f"{TAG} DRY RUN — would translate via {model}")
        return True

    # Read source
    try:
        with open(source_path, encoding="utf-8") as f:
            source_json = json.load(f)
    except (json.JSONDecodeError, OSError) as e:
        print(f"{TAG} Failed to read source: {e}")
        return False

    # Initialize client
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print(f"{TAG} OPENAI_API_KEY environment variable not set")
        return False
    client = openai.OpenAI(api_key=api_key)

    # Translate with retries
    translated = None
    for attempt in range(1, max_retries + 1):
        print(f"{TAG} Attempt {attempt}/{max_retries}...")
        try:
            translated = call_api(client, model, source_json, target_lang, locale)
            break
        except Exception as e:
            print(f"{TAG} Attempt {attempt} failed: {e}")
            if attempt < max_retries:
                wait = 2**attempt
                print(f"{TAG} Retrying in {wait}s...")
                time.sleep(wait)

    if translated is None:
        print(f"{TAG} FAILED after {max_retries} attempts")
        return False

    # Remap keys to ensure fidelity
    result, remapped, missing = remap_keys(source_json, translated)
    if remapped:
        print(f"{TAG} Remapped {len(remapped)} mutated key(s): {remapped}")
    if missing:
        print(f"{TAG} {len(missing)} key(s) kept as source (untranslated): {missing}")

    # Validate placeholders
    ph_errors = validate_placeholders(source_json, result)
    if ph_errors:
        print(f"{TAG} Placeholder mismatches:")
        for err in ph_errors:
            print(err)

    # Write sorted output
    result = dict(sorted(result.items()))
    outfile.parent.mkdir(parents=True, exist_ok=True)
    with open(outfile, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)
        f.write("\n")

    print(f"{TAG} Wrote {outfile} ({len(result)} keys)")
    return True


def main():
    parser = argparse.ArgumentParser(
        description="Translate rlvgl i18n JSON files using OpenAI GPT-5-nano"
    )
    parser.add_argument(
        "--locale",
        required=True,
        help="Target locale code (e.g. de, ja) or 'all' for all targets",
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=Path(__file__).resolve().parent / "locales" / "en.json",
        help="Path to source JSON (default: locales/en.json)",
    )
    parser.add_argument(
        "--outdir",
        type=Path,
        default=Path(__file__).resolve().parent / "locales",
        help="Output directory (default: locales/)",
    )
    parser.add_argument("--dry-run", action="store_true", help="Preview without translating")
    parser.add_argument("--force", action="store_true", help="Overwrite existing translations")
    parser.add_argument("--max-retries", type=int, default=3, help="Retry attempts (default: 3)")
    parser.add_argument("--model", default=DEFAULT_MODEL, help=f"OpenAI model (default: {DEFAULT_MODEL})")

    args = parser.parse_args()

    if args.locale == "all":
        locales = sorted(LOCALE_TO_LANGUAGE.keys())
    else:
        locales = [args.locale]

    ok = True
    for locale in locales:
        print()
        success = translate_locale(
            source_path=args.source,
            locale=locale,
            outdir=args.outdir,
            dry_run=args.dry_run,
            force=args.force,
            max_retries=args.max_retries,
            model=args.model,
        )
        if not success:
            ok = False

    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
