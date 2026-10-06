# SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Compare actual CLI/browser APK bytes, ZIP payloads, and Android signatures."""
import hashlib
import json
import os
import re
from pathlib import Path
import struct
import subprocess
import sys
import zipfile

def digest(path, start=0, length=None):
    result = hashlib.sha256()
    with path.open('rb') as source:
        source.seek(start)
        while length is None or length:
            chunk = source.read(min(length, 1 << 20) if length is not None else 1 << 20)
            if not chunk:
                break
            result.update(chunk)
            if length is not None:
                length -= len(chunk)
    return result.hexdigest()

def block(path, central):
    with path.open('rb') as source:
        source.seek(central - 24)
        footer = source.read(24)
    if footer[8:] != b'APK Sig Block 42':
        raise ValueError(f'{path}: APK signing block is missing')
    size = struct.unpack_from('<Q', footer)[0] + 8
    if size > central:
        raise ValueError(f'{path}: invalid APK signing block size')
    return central - size, central

def compare(a, b):
    with zipfile.ZipFile(a) as left, zipfile.ZipFile(b) as right:
        names_a, names_b = set(left.namelist()), set(right.namelist())
        differences = []
        for name in sorted(names_a & names_b):
            with left.open(name) as first, right.open(name) as second:
                while True:
                    x, y = first.read(1 << 20), second.read(1 << 20)
                    if x != y:
                        differences.append(name)
                        break
                    if not x:
                        break
        start_a, end_a = block(a, left.start_dir)
        start_b, end_b = block(b, right.start_dir)
        outside = start_a == start_b and end_a == end_b and digest(a, 0, start_a) == digest(b, 0, start_b) and digest(a, end_a) == digest(b, end_b)
    verify = []
    signer = os.environ.get('APKSIGNER', 'apksigner')
    for path in [a, b]:
        declared = subprocess.run([signer, 'verify', '--verbose', str(path)], capture_output=True, text=True)
        check = subprocess.run([signer, 'verify', '--min-sdk-version', '24', '--verbose', '--print-certs', str(path)], capture_output=True, text=True)
        verify.append({'valid': check.returncode == 0, 'minimum_verified_sdk': 24,
                       'certificate_sha256': re.findall(r'certificate SHA-256 digest: ([0-9a-f]+)', check.stdout),
                       'declared_min_sdk_valid': declared.returncode == 0,
                       'details': check.stdout + check.stderr, 'declared_min_sdk_details': declared.stdout + declared.stderr})
    return {'file': a.name, 'whole_file_identical': digest(a) == digest(b), 'bytes_outside_signing_block_identical': outside,
            'zip_entries': len(names_a), 'only_cli': sorted(names_a - names_b), 'only_browser': sorted(names_b - names_a),
            'different_entries': differences, 'same_signing_certificates': bool(verify[0]['certificate_sha256']) and verify[0]['certificate_sha256'] == verify[1]['certificate_sha256'], 'signatures': verify, 'cli_sha256': digest(a), 'browser_sha256': digest(b)}

cli, browser = (Path(value) for value in sys.argv[1:])
left = {path.relative_to(cli): path for path in cli.rglob('*.apk')}
right = {path.relative_to(browser): path for path in browser.rglob('*.apk')}
results = [compare(left[name], right[name]) for name in sorted(left.keys() & right.keys())]
report = {'apks': results, 'only_cli': [str(name) for name in left.keys() - right.keys()], 'only_browser': [str(name) for name in right.keys() - left.keys()]}
print(json.dumps(report))
passed = bool(results) and left.keys() == right.keys() and all(row['same_signing_certificates'] and row['bytes_outside_signing_block_identical'] and not row['different_entries'] and not row['only_cli'] and not row['only_browser'] and all(sign['valid'] for sign in row['signatures']) for row in results)
sys.exit(0 if passed else 1)
