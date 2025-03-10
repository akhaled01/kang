pub fn parse_size(size: &str) -> Option<u64> {
    if size.is_empty() {
        return None;
    }

    let (num, suffix) = if size
        .chars()
        .last()
        .map(|c| c.is_alphabetic())
        .unwrap_or(false)
    {
        size.split_at(size.len() - 1)
    } else {
        (size, "")
    };

    if let Ok(value) = num.parse::<u64>() {
        return match suffix {
            "K" | "k" => Some(value * 1_000),
            "M" | "m" => Some(value * 1_000_000),
            "G" | "g" => Some(value * 1_000_000_000),
            "" => Some(value),
            _ => None,
        };
    }

    None
}

pub fn draw_ascii() {
    let kang = r#"
:::    :::     :::     ::::    :::  :::::::: 
:+:   :+:    :+: :+:   :+:+:   :+: :+:    :+:
+:+  +:+    +:+   +:+  :+:+:+  +:+ +:+       
+#++:++    +#++:++#++: +#+ +:+ +#+ :#:       
+#+  +#+   +#+     +#+ +#+  +#+#+# +#+   +#+#
#+#   #+#  #+#     #+# #+#   #+#+# #+#    #+#
###    ### ###     ### ###    ####  ######## 
"#;
    println!("{}", kang);
}
