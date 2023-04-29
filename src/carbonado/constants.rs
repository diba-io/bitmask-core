use percent_encoding::{AsciiSet, CONTROLS};

// Based on the W3C spec: https://url.spec.whatwg.org/#percent-encoded-bytes
// Will try to contribute this upstream to `percent_encoding`

pub const FRAGMENT: &AsciiSet = &CONTROLS
    // U+0020 SPACE
    .add(b' ')
    // U+0022 (")
    .add(b'"')
    // U+003C (<)
    .add(b'<')
    // U+003E (>)
    .add(b'>')
    // U+0060 (`)
    .add(b'`');

pub const QUERY: &AsciiSet = &CONTROLS
    // U+0020 SPACE
    .add(b' ')
    // U+0022 (")
    .add(b'"')
    // U+0023 (#)
    .add(b'#')
    // U+003C (<)
    .add(b'<')
    // U+003E (>)
    .add(b'>');

pub const SPECIAL_QUERY: &AsciiSet = &QUERY
    // U+0027 (')
    .add(b'\'');

pub const PATH: &AsciiSet = &QUERY
    // U+003F (?)
    .add(b'?')
    // U+0060 (`)
    .add(b'`')
    // U+007B ({)
    .add(b'{')
    // U+007D (})
    .add(b'}');

pub const USERINFO: &AsciiSet = &PATH
    // U+002F (/)
    .add(b'/')
    // U+003A (:)
    .add(b':')
    // U+003B (;)
    .add(b';')
    // U+003D (=)
    .add(b'=')
    // U+0040 (@)
    .add(b'@')
    // U+005B ([)
    .add(b'[')
    // U+005C (\)
    .add(b'\\')
    // U+005D (])
    .add(b']')
    // U+005E (^)
    .add(b'^')
    // U+007C (|)
    .add(b'|');

pub const COMPONENT: &AsciiSet = &USERINFO
    // U+0024 ($)
    .add(b'$')
    // U+0025 (%)
    .add(b'%')
    // U+0026 (&)
    .add(b'&')
    // U+002B (+)
    .add(b'+')
    // U+002C (,)
    .add(b',');

pub const FORM: &AsciiSet = &COMPONENT
    // U+0021 (!)
    .add(b'!')
    // U+0027 (')
    .add(b'\'')
    // U+0028 LEFT PARENTHESIS
    .add(b'(')
    // U+0029 RIGHT PARENTHESIS
    .add(b')')
    // and U+007E (~)
    .add(b'~');
