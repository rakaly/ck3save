/*!
CK3 save files can be encoded in 4 different formats:

 - autosave (binary / text)
 - standard
 - ironman

Let's start with standard and ironman first. These two are similar in that there are three
sections:

 - a save id line
 - the header
 - a zip with the compressed gamestate

For standard saves, the header and the compressed gamestate are plaintext. Ironman saves use the
standard PDS binary format (not explained here).

What is interesting is that the gamestate contains the same header info. So one can bypass the
header and skip right to the zip file and there won't be any loss of data.

Now for autosave format:

 - a save id line
 - uncompressed gamestate in the binary or text format

These 4 formats pose an interesting challenge. If we only looked for the zip file signature (to
split the file to ensure our parser doesn't start interpretting zip data), we may end up scanning
100MB worth of data before realizing it's an autosave. This would be bad for performance. The
solution is to take advantage that zips orient themselves at the end of the file, so we assume
a zip until further notice.

In short, to know what the save file format:

- Attempt to parse as zip
- if not a zip, we know it's an autosave
- else if the 3rd and 4th byte are `01 00` then we know it's ironman
- else it's a standard save
*/

/// Describes the format of the save before decoding
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoding {
    /// Save is encoded with the standard format:
    ///
    ///  - a save id line
    /// - uncompressed binary gamestate
    Text,

    /// Save is encoded with the standard format:
    ///
    ///  - a save id line
    ///  - plaintext header
    ///  - zip with compressed plaintext gamestate
    TextZip,

    /// Save is encoded in the binary zip format
    ///
    ///  - a save id line
    ///  - binary header
    ///  - zip with compressed binary gamestate
    BinaryZip,

    /// Save is encoded in the binary format
    ///
    ///  - a save id line
    ///  - uncompressed binary gamestate
    Binary,
}
