/*
 * Copyright 2026 Memryzed contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * glibc compatibility shim for older Linux systems.
 *
 * The prebuilt ONNX Runtime that `ort` downloads is compiled against
 * glibc that exports the C23 `__isoc23_strto*` variants, introduced
 * in glibc 2.38 (August 2023). On older systems (Ubuntu 22.04 ships
 * glibc 2.35) the linker fails with an "undefined symbol" error.
 *
 * Each symbol is declared `__attribute__((weak))` so on glibc 2.38+
 * the strong symbol from glibc takes precedence and these
 * definitions are ignored. On glibc < 2.38 these stubs satisfy the
 * linker and forward to the corresponding non-C23 symbol.
 *
 * Once the project's minimum supported glibc is 2.38 or newer,
 * delete this file and the corresponding `cc::Build` invocation in
 * `build.rs`.
 */

#include <stdlib.h>

__attribute__((weak)) long __isoc23_strtol(const char *nptr, char **endptr,
                                           int base) {
  return strtol(nptr, endptr, base);
}

__attribute__((weak)) long long __isoc23_strtoll(const char *nptr,
                                                 char **endptr, int base) {
  return strtoll(nptr, endptr, base);
}

__attribute__((weak)) unsigned long long
__isoc23_strtoull(const char *nptr, char **endptr, int base) {
  return strtoull(nptr, endptr, base);
}
