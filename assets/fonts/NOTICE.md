# Bundled fonts

This directory ships the following fonts as compile-time fallbacks for the
default egui font stack, so chat binds that include math-style or
dingbat-style Unicode (e.g. `𝕊𝕔𝕠𝕠𝕡𝕤`, `✿`) render correctly instead of
showing tofu boxes.

| File                              | Source                                                                            | Copyright                                              |
|-----------------------------------|-----------------------------------------------------------------------------------|--------------------------------------------------------|
| `NotoSansMath-Regular.ttf`        | <https://github.com/notofonts/math>                                                | Copyright 2022 The Noto Project Authors                |
| `NotoSansSymbols2-Regular.ttf`    | <https://github.com/notofonts/symbols2>                                            | Copyright 2022 The Noto Project Authors                |

Both fonts are distributed under the **SIL Open Font License, Version 1.1**
(see `OFL.txt`). The OFL permits redistribution, modification, and
embedding in other software, including this binary, provided the license
text travels with the fonts and the fonts are not sold by themselves.
