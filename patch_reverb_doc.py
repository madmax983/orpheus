import re

with open('crates/orpheus-lang/src/session.rs', 'r') as f:
    content = f.read()

# Let's check what tests failed in session.rs
#     crates/orpheus-lang/src/session.rs - session::MixerView (line 119)
#     crates/orpheus-lang/src/session.rs - session::ReplSession::mixer_view (line 1371)
