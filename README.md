# Export training sessions from Polar Flow website

[![Build Status](https://travis-ci.org/scanban/polar-flow-export.svg?branch=master)](https://travis-ci.org/scanban/polar-flow-export)
![GitHub](https://img.shields.io/github/license/scanban/polar-flow-export.svg)

## examples

export all sessions in tcx format to the zip archive `c:\exports\polar_exports.zip`:
```
polar_export.exe -u my@email.com -p my-password zip -o c:\exports\polar_exports.zip
```

## usage

```
USAGE:
    polar_export.exe [FLAGS] -u <EMAIL> -e <DATE> -f <EXPORT-FORMAT> -p <PASSWORD> -s <DATE> [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -v               Sets the level of verbosity
    -V, --version    Prints version information

OPTIONS:
    -u <EMAIL>                Polar flow registration email
    -e <DATE>                 End date for export, format DD.MM.YYYY [default: 31.12.2039]
    -f <EXPORT-FORMAT>        Training sessions export format [default: tcx]  [possible values: tcx, gpx, csv]
    -p <PASSWORD>             Polar flow registration password
    -s <DATE>                 Start date for export, format DD.MM.YYYY [default: 01.01.1970]

SUBCOMMANDS:
    files    exports all sessions into directory
    help     Prints this message or the help of the given subcommand(s)
    zip      exports all sessions into zip archive

```

## Credits / Thank you

* [`jhujhul`], JS based export utility creator

[`jhujhul`]: https://github.com/jhujhul