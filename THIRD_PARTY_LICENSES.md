# Third-party notices

The production backend uses jiter 0.16.0 from the Pydantic project. jiter is
available under the MIT license. The repository does not enable its Python
feature.

The production backend uses speedate 0.17.0 from the Pydantic project.
speedate is available under the MIT license.

The production backend uses base64 0.22.1. The crate is available under the
MIT or Apache-2.0 licenses.

The production backend uses the Sifr structural runtime and identity crates
from commit `4f5492531e81385dd28efe25adfdd57dd678d2a9`. Sifr is available under
the MIT license. The exact commit pin prevents a mutable tag from changing the
dependency.

The Sifr structural feature uses encoding_rs 0.8.35, indexmap 2.14.0,
hashbrown 0.17.1, equivalent 1.0.2, sha2 0.10.9, digest 0.10.7,
block-buffer 0.10.4, crypto-common 0.1.7, generic-array 0.14.7, typenum 1.20.1,
and cpufeatures 0.2.17. These crates are available under the MIT license, the
Apache-2.0 license, or both. encoding_rs also includes BSD-3-Clause material.

The production backend uses url 2.5.8 and uuid 1.24.0. These crates are
available under the MIT or Apache-2.0 licenses. The url dependency uses idna
and ICU4X crates. ICU4X data and components include material under the
Unicode License Version 3.0. Keep their license and copyright notices with
distributions of this software.

The test provenance tools inspect Pydantic and its in-tree Pydantic Core at
commit f59e929c999e8b2efc7b12fd0bc1685c1a186be3. Both projects are available
under the MIT license. No upstream production implementation is copied or
linked into pydantic-sifr.
