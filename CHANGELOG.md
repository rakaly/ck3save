## v0.2.2 - 2021-03-14

- Bump internal parser to latest

## v0.2.1 - 2021-02-05

- Melter will only quote values that are quoted in plaintext

## v0.2.0 - 2021-01-25

* Fixed seed properties being detected and melted as dates instead of numbers
* *Breaking*: Melter will return a set of unknown tokens (when melting does not fail)

## v0.1.4 - 2020-10-29

* Update internal parser for performance improvements

## v0.1.3 - 2020-10-02

* Fix botched release

## v0.1.2 - 2020-10-02

* Update parser dependency to 0.7
* Able to losslessly melt `levels = { 10 0=1 1=2 }`

## v0.1.1 - 2020-09-12

Update internal parser to latest which brings proper UTF-8 deserialization, performance improvements, and robustness against malicious input

## v0.1.0 - 2020-09-07

Initial commit with basic extraction and melting capabilities
