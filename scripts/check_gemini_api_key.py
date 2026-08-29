#!/usr/bin/env python3
"""Validate the local Gemini API key without displaying it.

Usage:
    python scripts/check_gemini_api_key.py
    python scripts/check_gemini_api_key.py --env-file src-tauri/.env
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


def load_key(env_file: Path) -> str | None:
    key = os.environ.get("GEMINI_API_KEY", "").strip()
    if key:
        return key

    if not env_file.is_file():
        return None

    for raw_line in env_file.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        name, value = line.split("=", 1)
        if name.strip() == "GEMINI_API_KEY":
            return value.strip().strip('"').strip("'") or None
    return None


def response_message(payload: bytes) -> str:
    try:
        error = json.loads(payload.decode("utf-8"))["error"]
        return str(error.get("message", "No error message returned by Gemini."))
    except (json.JSONDecodeError, KeyError, UnicodeDecodeError):
        return payload.decode("utf-8", errors="replace").strip() or "No error message returned by Gemini."


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate a Gemini API key using the model-list endpoint.")
    parser.add_argument("--env-file", default="src-tauri/.env", help="Path to a .env file (default: src-tauri/.env)")
    parser.add_argument("--model", default="gemini-3.5-flash-lite", help="Model to verify (default: gemini-3.5-flash-lite)")
    parser.add_argument("--smoke-test", action="store_true", help="Send a minimal generation request to the selected model")
    args = parser.parse_args()

    key = load_key(Path(args.env_file))
    if not key:
        print("GEMINI_API_KEY is missing or empty.")
        return 2

    url = "https://generativelanguage.googleapis.com/v1beta/models?" + urlencode({"key": key})
    try:
        with urlopen(url, timeout=15) as response:
            payload = json.load(response)
    except HTTPError as error:
        message = response_message(error.read())
        print(f"Gemini rejected the request (HTTP {error.code}): {message}")
        if error.code == 400:
            print("Result: the API key is invalid, malformed, or no longer active.")
        elif error.code == 403:
            print("Result: the key exists, but this project, API, or key restriction blocks Gemini access.")
        return 1
    except URLError as error:
        print(f"Could not reach the Gemini API: {error.reason}")
        return 3

    models = payload.get("models", [])
    model_names = {model.get("name", "").removeprefix("models/") for model in models}
    print(f"Gemini API key is valid. The project can access {len(model_names)} model(s).")
    if args.model not in model_names:
        print(f"{args.model} is not listed for this key; use a model returned by this endpoint.")
        return 1

    print(f"{args.model} is available to this key.")
    if not args.smoke_test:
        return 0

    body = json.dumps({
        "contents": [{"role": "user", "parts": [{"text": "Reply with OK."}]}],
        "generationConfig": {"maxOutputTokens": 8, "temperature": 0},
    }).encode("utf-8")
    request = Request(
        f"https://generativelanguage.googleapis.com/v1beta/models/{args.model}:generateContent?" + urlencode({"key": key}),
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urlopen(request, timeout=30) as response:
            response.read()
    except HTTPError as error:
        print(f"{args.model} rejected the generation request (HTTP {error.code}): {response_message(error.read())}")
        return 1
    except URLError as error:
        print(f"Could not reach the Gemini API for the generation test: {error.reason}")
        return 3

    print(f"{args.model} accepted a generation request.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
