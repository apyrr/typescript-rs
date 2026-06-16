use super::{try_parse_version, try_parse_version_range};

struct TestForRangeOnVersion<'a> {
    range_text: &'a str,
    version_text: &'a str,
    expected: bool,
}

const COMPARATORS_TESTS: &[TestForRangeOnVersion] = &[
    TestForRangeOnVersion {
        range_text: "",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "*",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.1.0-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.1-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=*",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.1.0-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.1-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "=1.0.0-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">*",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.1.0-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.1-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=*",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.1.0-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.1-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0-0",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<*",
        version_text: "0.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.1.0-0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.1-0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "1.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<1.0.0-0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "2.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=*",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "1.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "1.1.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.1.0-0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "1.0.1-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.1-0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "2.0.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "1.1.0-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "1.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "1.0.1-0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "1.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<=1.0.0-0",
        version_text: "0.0.0-0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">4.8",
        version_text: "4.9.0-beta",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=4.9",
        version_text: "4.9.0-beta",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "<4.9",
        version_text: "4.9.0-beta",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "<=4.8",
        version_text: "4.9.0-beta",
        expected: false,
    },
];

const CONJUNCTION_TESTS: &[TestForRangeOnVersion] = &[
    TestForRangeOnVersion {
        range_text: ">1.0.0 <2.0.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0 <2.0.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0 <2.0.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1 >2",
        version_text: "3.0.0",
        expected: true,
    },
];

const DISJUNCTION_TESTS: &[TestForRangeOnVersion] = &[
    TestForRangeOnVersion {
        range_text: ">1.0.0 || <1.0.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0 || <1.0.0",
        version_text: "0.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0 || <1.0.0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">1.0.0 || <1.0.0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0 <2.0.0 || >=3.0.0 <4.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0 <2.0.0 || >=3.0.0 <4.0.0",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: ">=1.0.0 <2.0.0 || >=3.0.0 <4.0.0",
        version_text: "3.0.0",
        expected: true,
    },
];

const HYPHEN_TESTS: &[TestForRangeOnVersion] = &[
    TestForRangeOnVersion {
        range_text: "1.0.0 - 2.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0 - 2.0.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0 - 2.0.0",
        version_text: "2.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0 - 2.0.0",
        version_text: "2.0.1",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0 - 2.0.0",
        version_text: "0.9.9",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "1.0.0 - 2.0.0",
        version_text: "3.0.0",
        expected: false,
    },
];

const TILDE_TESTS: &[TestForRangeOnVersion] = &[
    TestForRangeOnVersion {
        range_text: "~0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0",
        version_text: "0.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0",
        version_text: "0.1.2",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0",
        version_text: "0.1.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~0.1",
        version_text: "0.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0.1",
        version_text: "0.1.2",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0.1",
        version_text: "0.1.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0.1",
        version_text: "0.2.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~0.1.2",
        version_text: "0.1.2",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0.1.2",
        version_text: "0.1.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~0.1.2",
        version_text: "0.2.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~1.0.0",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1.0.0",
        version_text: "1.0.1",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1",
        version_text: "1.2.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1",
        version_text: "1.2.3",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~1.2",
        version_text: "1.2.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1.2",
        version_text: "1.2.3",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1.2",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~1.2",
        version_text: "1.3.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~1.2.3",
        version_text: "1.2.3",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1.2.3",
        version_text: "1.2.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "~1.2.3",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "~1.2.3",
        version_text: "1.3.0",
        expected: false,
    },
];

const CARET_TESTS: &[TestForRangeOnVersion] = &[
    TestForRangeOnVersion {
        range_text: "^0",
        version_text: "0.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0",
        version_text: "0.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0",
        version_text: "0.9.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0",
        version_text: "0.1.2",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0",
        version_text: "0.1.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^0.1",
        version_text: "0.1.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0.1",
        version_text: "0.1.2",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0.1",
        version_text: "0.1.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0.1.2",
        version_text: "0.1.2",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0.1.2",
        version_text: "0.1.9",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^0.1.2",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^0.1.2",
        version_text: "0.2.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^0.1.2",
        version_text: "1.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^1",
        version_text: "1.0.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1",
        version_text: "1.2.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1",
        version_text: "1.2.3",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1",
        version_text: "1.9.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1",
        version_text: "0.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^1",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^1.2",
        version_text: "1.2.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1.2",
        version_text: "1.2.3",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1.2",
        version_text: "1.9.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1.2",
        version_text: "1.1.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^1.2",
        version_text: "2.0.0",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^1.2.3",
        version_text: "1.2.3",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1.2.3",
        version_text: "1.9.0",
        expected: true,
    },
    TestForRangeOnVersion {
        range_text: "^1.2.3",
        version_text: "1.2.2",
        expected: false,
    },
    TestForRangeOnVersion {
        range_text: "^1.2.3",
        version_text: "2.0.0",
        expected: false,
    },
];

#[test]
fn wildcards_have_same_string() {
    assert_all_version_ranges_have_identical_strings(&[
        "", "*", "*.*", "*.*.*", "x", "x.x", "x.x.x", "X", "X.X", "X.X.X",
    ]);
    assert_all_version_ranges_have_identical_strings(&[
        "1", "1.*", "1.*.*", "1.x", "1.x.x", "1.X", "1.X.X",
    ]);
    assert_all_version_ranges_have_identical_strings(&["1.2", "1.2.*", "1.2.x", "1.2.X"]);
    assert_all_version_ranges_have_identical_strings(&["x", "X", "*", "x.X.x", "X.x.*"]);
}

#[test]
fn version_ranges_match_good_and_bad_versions() {
    assert_ranges_good_bad(
        "1",
        &["1.0.0", "1.9.9", "1.0.0-pre", "1.0.0+build"],
        &["0.0.0", "2.0.0", "0.0.0-pre", "0.0.0+build"],
    );
    assert_ranges_good_bad(
        "1.2",
        &["1.2.0", "1.2.9", "1.2.0-pre", "1.2.0+build"],
        &["1.1.0", "1.3.0", "1.1.0-pre", "1.1.0+build"],
    );
    assert_ranges_good_bad(
        "1.2.3",
        &["1.2.3", "1.2.3+build"],
        &["1.2.2", "1.2.4", "1.2.2-pre", "1.2.2+build", "1.2.3-pre"],
    );
    assert_ranges_good_bad(
        "1.2.3-pre",
        &["1.2.3-pre", "1.2.3-pre+build.stuff"],
        &[
            "1.2.3",
            "1.2.3-pre.0",
            "1.2.3-pre.9",
            "1.2.3-pre.0+build",
            "1.2.3-pre.9+build",
            "1.2.3+build",
            "1.2.4",
        ],
    );
    assert_ranges_good_bad("<3.8.0", &["3.6", "3.7"], &["3.8", "3.9", "4.0"]);
    assert_ranges_good_bad("<=3.8.0", &["3.6", "3.7", "3.8"], &["3.9", "4.0"]);
    assert_ranges_good_bad(">3.8.0", &["3.9", "4.0"], &["3.6", "3.7", "3.8"]);
    assert_ranges_good_bad(">=3.8.0", &["3.8", "3.9", "4.0"], &["3.6", "3.7"]);
    assert_ranges_good_bad("<3.8.0-0", &["3.6", "3.7"], &["3.8", "3.9", "4.0"]);
    assert_ranges_good_bad("<=3.8.0-0", &["3.6", "3.7"], &["3.8", "3.9", "4.0"]);

    let lotsa_ones = "1".repeat(320);
    let range = format!(">=1.2.3-1{lotsa_ones}");
    let good = [
        format!("1.2.3-1{lotsa_ones}"),
        format!("1.2.3-11{lotsa_ones}.1"),
        format!("1.2.3-1{lotsa_ones}.1+build"),
    ];
    let bad = [format!("1.2.3-{lotsa_ones}.1+build")];
    assert_ranges_good_bad(
        &range,
        &good.iter().map(String::as_str).collect::<Vec<_>>(),
        &bad.iter().map(String::as_str).collect::<Vec<_>>(),
    );
}

#[test]
fn comparators_of_version_ranges() {
    let comparator_tests = COMPARATORS_TESTS;
    for test in comparator_tests {
        assert_range_test(
            "comparators",
            test.range_text,
            test.version_text,
            test.expected,
        );
    }
}

#[test]
fn conjunctions_of_version_ranges() {
    let conjunction_tests = CONJUNCTION_TESTS;
    for test in conjunction_tests {
        assert_range_test(
            "conjunctions",
            test.range_text,
            test.version_text,
            test.expected,
        );
    }
}

#[test]
fn disjunctions_of_version_ranges() {
    let disjunction_tests = DISJUNCTION_TESTS;
    for test in disjunction_tests {
        assert_range_test(
            "disjunctions",
            test.range_text,
            test.version_text,
            test.expected,
        );
    }
}

#[test]
fn hyphens_of_version_ranges() {
    let hyphen_tests = HYPHEN_TESTS;
    for test in hyphen_tests {
        assert_range_test("hyphens", test.range_text, test.version_text, test.expected);
    }
}

#[test]
fn tildes_of_version_ranges() {
    let tilde_tests = TILDE_TESTS;
    for test in tilde_tests {
        assert_range_test("tilde", test.range_text, test.version_text, test.expected);
    }
}

#[test]
fn carets_of_version_ranges() {
    let caret_tests = CARET_TESTS;
    for test in caret_tests {
        assert_range_test("caret", test.range_text, test.version_text, test.expected);
    }
}

fn assert_all_version_ranges_have_identical_strings(strs: &[&str]) {
    for s1 in strs {
        for s2 in strs {
            let (v1, ok1) = try_parse_version_range(s1);
            assert!(ok1, "failed to parse range {s1:?}");
            let (v2, ok2) = try_parse_version_range(s2);
            assert!(ok2, "failed to parse range {s2:?}");
            assert_eq!(v1.to_string(), v2.to_string(), "{s1:?} != {s2:?}");
        }
    }
}

fn assert_ranges_good_bad(range: &str, good: &[&str], bad: &[&str]) {
    for version in good {
        assert_range_test(range, range, version, true);
    }
    for version in bad {
        assert_range_test(range, range, version, false);
    }
}

fn assert_range_test(_name: &str, range: &str, version: &str, expected: bool) {
    let (range, ok) = try_parse_version_range(range);
    assert!(ok, "failed to parse range");
    let version = try_parse_version(version).expect("failed to parse version");
    assert_eq!(range.test(&version), expected);
}
