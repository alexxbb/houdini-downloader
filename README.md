## API and command line utility for downloading Houdini builds from SideFX.

Based on _official_
[SideFX Web API](https://www.sidefx.com/docs/api/index.html)

## `houdl` command

The command checks for the `SESI_USER_ID` and `SESI_USER_SECRET` environment variable 
if not passed directly as arguments.

### Example: list builds
`>> houdl list --version 19.5 --platform macos`

```shell
 0. Date: 2023/11/21, Platform: linux_x86_64_gcc9.3, Version: 19.5.805, Status: good, Release: gold
 1. Date: 2023/10/19, Platform: linux_x86_64_gcc9.3, Version: 19.5.773, Status: good, Release: gold
 2. Date: 2023/09/28, Platform: linux_x86_64_gcc9.3, Version: 19.5.752, Status: good, Release: gold
 3. Date: 2023/08/23, Platform: linux_x86_64_gcc9.3, Version: 19.5.716, Status: good, Release: gold
 4. Date: 2023/07/24, Platform: linux_x86_64_gcc9.3, Version: 19.5.682, Status: good, Release: gold
 5. Date: 2023/06/08, Platform: linux_x86_64_gcc9.3, Version: 19.5.640, Status: good, Release: gold
 6. Date: 2023/05/04, Platform: linux_x86_64_gcc9.3, Version: 19.5.605, Status: good, Release: gold
 7. Date: 2023/03/29, Platform: linux_x86_64_gcc9.3, Version: 19.5.569, Status: good, Release: gold
 ...
```

### Example: download a particular Houdini build

`houdl --package houdini --platform macos --version 19.5 --build 805 --output-dir .`

```shell
✔ Download houdini-19.5.805-macosx_x86_64_clang12.0_11.dmg? · yes
Downloading houdini-19.5.805-macosx_x86_64_clang12.0_11.dmg
⠠ [00:00:18] [####################>-----------------] 1.05 GiB/1.93 GiB (56.39 MiB/s, 16s)
Build md5 checksum: f355bfe7271e0755908a3680f1f3c619
```
To make sure the download is valid the md5 hash of the downloaded bytes is computed and verified.

**(checksum can also be found on the download page next to a build)**

## Build & Run
1. Have a Rust toolchain installed: https://rustup.rs/
2. Obtain a user id & key from the SideFX Web API page.
3. Clone the repository
4. `cargo run`
