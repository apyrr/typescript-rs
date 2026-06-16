use crate::{new_fourslash, TestingT};

pub fn test_chinese_character_display_in_hover(t: &mut TestingT) {
    let content = r#"
interface 中文界面 {
    上居中: string;
    下居中: string;
}

class 中文类 {
    获取中文属性(): 中文界面 {
        return {
            上居中: "上居中",
            下居中: "下居中"
        };
    }
}

let /*instanceHover*/实例 = new 中文类();
let 属性对象 = 实例./*methodHover*/获取中文属性();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "instanceHover", "let 实例: 中文类", "");
    f.verify_quick_info_at(t, "methodHover", "(method) 中文类.获取中文属性(): 中文界面", "");
    done();
}

pub fn test_chinese_character_display_in_union_types(t: &mut TestingT) {
    let content = r#"
// Test the original issue: Chinese characters in method parameters should display correctly
class TSLine {
    setLengthTextPositionPreset(/*methodParam*/preset: "上居中" | "下居中" | "右居中" | "左居中"): void {}
}

let lines = new TSLine();
lines./*method*/setLengthTextPositionPreset;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Verify that the method displays Chinese characters correctly in hover (this was the original problem)
    f.verify_quick_info_at(
        t,
        "method",
        r#"(method) TSLine.setLengthTextPositionPreset(preset: "上居中" | "下居中" | "右居中" | "左居中"): void"#,
        "",
    );
    done();
}

