# guzuta
Custom repository manager for ArchLinux pacman

## Usage
### Build a package and store it to a repository
Basic usage: build foo x86_64 package for bar repository.

```
% ls foo
PKGBUILD
% mkdir -p repo/x86_64
% guzuta build foo --repo-dir repo/x86_64 --repo-name bar --arch x86_64
(snip)
% ls repo/x86_64
bar.abs.tar.gz  bar.db  bar.files  foo-1.0.0-1-x86_64.pkg.tar.xz
```

With full options:
```
% guzuta build --chroot-dir /var/cache/guzuta/chroot-x86_64 --repo-dir repo/x86_64 --repo-name bar --arch x86_64 --package-key $GPGKEY --repo-key $GPGKEY --srcdest sources --logdest logs foo
(snip)
% ls repo/x86_64
bar.abs.tar.gz  bar.db  bar.db.sig  bar.files  bar.files.sig  foo-1.0.0-1-x86_64.pkg.tar.xz  foo-1.0.0-1-x86_64.pkg.tar.xz.sig
% ls sources
foo-1.0.0.tar.gz
% ls logs
foo-1.0.0-1-x86_64-build.log  foo-1.0.0-1-x86_64-package.log
```

## Omakase mode
Omakase mode supports a typical situation managing the custom repository.

### Initialize a repository

```
% mkarchroot -C /path/to/pacman.conf -M /path/to/makepkg.conf /var/cache/guzuta/chroot-x86_64
% cat > .guzuta.yml
name: foo
package_key: C48DBD97
repo_key: C48DBD97
srcdest: sources
logdest: logs
pkgbuild: PKGBUILDs
builds:
  x86_64:
    chroot: /path/to/chroot-x86_64
% mkdir foo sources logs PKGBUILDs
```

### Build a package
Write a PKGBUILD in `PKGBUILDs/#{pkgname}` directory.

```
% mkdir PKGBUILDs/bar
% vim PKGBUILDs/bar/PKGBUILD
```

Then build the package.

```
% guzuta omakase build bar
(snip)
% tree foo
foo
`-- os
    `-- x86_64
        |-- bar-1.0.0-1-x86_64.pkg.tar.xz
        |-- bar-1.0.0-1-x86_64.pkg.tar.xz.sig
        |-- foo.abs.tar.gz
        |-- foo.db
        |-- foo.db.sig
        `-- foo.files
```

### Publish the repository
For the server, serve files under the foo directory by HTTP server like nginx or Apache.

For clients, add the server's repository configuration to /etc/pacman.conf like below.

```
[foo]
SigLevel = Required
Server = http://example.com/$repo/os/$arch
```

### Publish the repository (Amazon S3)
Configure .guzuta.yml for S3.

```yaml
s3:
  bucket: foo-packages
  region: ap-northeast-1
```

Each time you execute `guzuta omakase build`:

1. Download repository databases (not including packages)
2. Build a package
3. Upload the built package and repository databases.

