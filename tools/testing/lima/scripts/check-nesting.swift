// SPDX-License-Identifier: Apache-2.0

import Foundation
import Virtualization

if #available(macOS 15.0, *) {
    guard VZGenericPlatformConfiguration.isNestedVirtualizationSupported else {
        fputs("Nested virtualization requires an Apple M3 or later Mac and macOS 15 or newer. M1, M2, and Intel Macs cannot run this local Kata lab; use a remote Linux execution host.\n", stderr)
        exit(1)
    }
    print("Apple Virtualization.framework supports nested virtualization on this Mac")
} else {
    fputs("The Lima Kata target requires macOS 15 or newer for nested virtualization.\n", stderr)
    exit(1)
}
