# Private development preview packaging

Run from a Windows checkout with Git, Rust/Cargo and PowerShell available:

```powershell
pwsh -File .\tools\package-preview.ps1
```

[The script](../tools/package-preview.ps1) always runs `cargo build --release --locked`. It takes the executable path from Cargo's artifact output rather than assuming a target directory or reusing an arbitrary executable. Finish concurrent Rust edits before packaging: changed local build inputs or a changed HEAD during the build stop packaging. Re-run after worker integration before treating an artifact as the current candidate.

Each run writes a new unique directory under `target/preview`; it does not delete previous artifacts. The ZIP contains exactly `tracex.exe`, an honest private-preview `README.txt`, `manifest.json`, and `SHA256SUMS.txt`. A sibling `.zip.sha256` records the archive hash. A distinct `verification-extract` directory preserves the extracted verification copy. The script verifies its four filenames, payload checksums and executable equality with the Cargo artifact. It does not launch the app, sign, upload, publish or install anything.

The manifest records package version, PE-derived executable architecture, base commit, dirty state, SHA-256 of tracked Git diff plus untracked path/content-hash records, and a separate repository-local build-input fingerprint. The latter covers Rust files, `src/`, Cargo manifests/lockfile, `.cargo/`, and toolchain files. A dirty build is explicitly identified as local modifications on a base commit. Neither fingerprints nor a locked build establish bit-for-bit reproducibility or a clean-commit artifact. Tool versions, build timestamps and lockfile hash aid reproduction; retain the working source separately. External Cargo configuration, environment and system linker/SDK are not exhaustively captured. No source diff, device logs, credentials or user diagnostics are added to the ZIP.

The current package README marks the artifact unsigned and private, describes shared monitor capture, unbounded retention/queues, potential blocking I/O and limited durability, and supplies no redistribution grant or third-party bundle. Public mode includes the separate binary terms and third-party notice/source bundle; do not infer that a private package authorizes public distribution.

No paid signing is required to create this private artifact. Windows may display unsigned/unrecognized publisher warnings. Do not bypass SmartScreen or other OS protection; document the unsigned status and use the normal security review process. SHA-256 is an integrity comparison, not publisher authentication. Public development-preview publication is a separate authorized delivery step once licensing, limitations, actual artifact and checks are ready; production readiness must never be implied.

Packaging is additional evidence, not the repository quality gate. Run formatting, Clippy, tests and release build per [AGENTS.md](../AGENTS.md). Then verify launch and the relevant device workflows on supported Windows before broad testing. This script creates no installer/update/uninstall or hardware-test evidence.

## Public development preview mode

After the coordinating release owner has accepted integration checks:

```powershell
pwsh -File .\tools\package-preview.ps1 -PublicDevelopmentPreview
```

Private packaging remains the default. Public mode requires [BINARY-LICENSE.txt](../distribution/BINARY-LICENSE.txt), [third-party notices](../distribution/THIRD-PARTY-NOTICES.txt), an inventory matching the current lockfile, per-package notices and verified bundled source archives. It supports the inventoried Windows x86_64 target only. It adds `BINARY-LICENSE.txt` and `THIRD-PARTY/` to the package and hashes every included file. The script prepares and verifies a local development-preview ZIP; publication remains a separate action.

The conservative Windows metadata inventory contains 174 registry packages, including packages that may be build-only or optional in the particular backend. Complete supplied license texts and notices, font terms and font-embedded notices are retained. Both MPL components (`serialport 4.8.1`, `option-ext 0.2.0`) have exact original `.crate` sources bundled and verified against `Cargo.lock`, with a source-availability notice. Additional source archives retain notices for crates omitting standalone license files. Upstream-retrieved license texts have source URLs recorded; some egui-family texts use the upstream 0.30.0 release tag because packaged VCS revisions are unavailable. Dependency changes require refreshing this inventory and reviewing new terms. This workflow does not certify legal compliance or imply production readiness.

Rust standard-library/runtime notices, including the toolchain's complete `COPYRIGHT-library.html` and adjacent license texts, are bundled separately from Cargo dependencies. Public mode requires the recorded `rustc -vV` to match the current compiler; refresh that bundle when changing toolchain. The workflow does not bundle a compiler or install a runtime redistributable.

The inspected Windows executable imports `VCRUNTIME140.dll`. This build requires the Microsoft Visual C++ v14 x64 runtime, version 14.44 or newer. The package README links [Microsoft's supported runtime documentation](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/) and [official latest x64 runtime](https://aka.ms/vc14/vc_redist.x64.exe). This workflow does not download, bundle, install or change CRT linkage. Recheck the prerequisite after changing compiler/linker toolchains; a clean-machine launch remains a separate gate.
