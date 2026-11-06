#!/usr/bin/env python3
# Copyright (c) 2026 Kata Contributors
# SPDX-License-Identifier: Apache-2.0

"""Render both real Makefile recipes in temporary files and check TOML scoping.

Run with Python 3.11+ on Linux/WSL; no VMM or installed guest image is needed.
"""

from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[3]
APPARMOR_BLOCK = re.compile(
    r'(?m)^#\[\[hypervisor\.qemu\.guest_extension_images\]\]\n'
    r'#name = "apparmor"\n#path = "[^"]+"\n#verity_params = "[^"]+"'
)


class AppArmorConfigurationTest(unittest.TestCase):
    def test_generated_configurations(self):
        variants = [
            ([], "/usr/share/kata-containers/kata-containers-apparmor-extension.img"),
            (
                ["PREFIX=/opt/kata-test", "IMAGENAME=different-base.img"],
                "/opt/kata-test/share/kata-containers/kata-containers-apparmor-extension.img",
            ),
            (
                ["APPARMOREXTENSIONPATH=/custom/policy.img"],
                "/custom/policy.img",
            ),
        ]
        for runtime, template in [
            ("runtime", "configuration-qemu.toml.in"),
            ("runtime-rs", "configuration-qemu-runtime-rs.toml.in"),
        ]:
            source = ROOT / "src" / runtime
            for variables, expected_path in variants:
                with self.subTest(runtime=runtime, variables=variables):
                    with tempfile.TemporaryDirectory(prefix="kata-apparmor-config-") as tmp:
                        output = Path(tmp) / "configuration.toml"
                        shutil.copyfile(source / "config" / template, str(output) + ".in")
                        # Override only the generated output list. -o prevents the
                        # Go recipe's VERSION/.git-commit prerequisites being written
                        # in this shared checkout; neither value affects this test.
                        subprocess.run(
                            [
                                "make", "--no-print-directory", "-s",
                                "-o", "VERSION", "-o", ".git-commit",
                                f"GENERATED_FILES={output}", f"CONFIGS={output}",
                                "SKIP_GO_VERSION_CHECK=1", *variables, str(output),
                            ],
                            cwd=source,
                            check=True,
                            capture_output=True,
                            text=True,
                        )
                        rendered = output.read_text()
                    self.assertIsNone(re.search(r"@[A-Z][A-Z0-9_]*@", rendered))
                    baseline = tomllib.loads(rendered)
                    self.assertNotIn("guest_extension_images", baseline["hypervisor"]["qemu"])
                    activated, count = APPARMOR_BLOCK.subn(
                        lambda match: "\n".join(line[1:] for line in match[0].splitlines()),
                        rendered,
                    )
                    self.assertEqual(count, 1, "expected exactly one AppArmor example")
                    enabled = tomllib.loads(activated)
                    extensions = enabled["hypervisor"]["qemu"].pop("guest_extension_images")
                    self.assertEqual(len(extensions), 1)
                    self.assertEqual(extensions[0]["name"], "apparmor")
                    self.assertEqual(extensions[0]["path"], expected_path)
                    self.assertEqual(set(extensions[0]), {"name", "path", "verity_params"})
                    # Compare every section and QEMU scalar, including factory.
                    self.assertEqual(enabled, baseline)


if __name__ == "__main__":
    unittest.main(verbosity=2)
