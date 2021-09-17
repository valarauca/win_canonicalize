win_canonicalize

This canonicalizes paths for windows/mingw32/mingw64/cygwin.
It will also attempt to canonicalize the path if it exists or not.

This mean it will correct things like:

* `~` -> `${HOME}`.
* Normalize `/` and `\` usage.
* Resolve `..` & `.` runs.
* If you path uses `\` to escape, it might get broken. Idc to test this.

### License

All rights reserved 2021 william cody laeder
