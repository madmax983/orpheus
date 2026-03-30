import re
with open('crates/orpheus-lang/tests/loader.rs', 'r') as f:
    data = f.read()

data = data.replace(
    '"Expected error for input \'{}\' to contain \'{}\', but got \'{}\'",\n            input,\n            expected_error,\n            error',
    '"Expected error for input \'{input}\' to contain \'{expected_error}\', but got \'{error}\'"'
)

with open('crates/orpheus-lang/tests/loader.rs', 'w') as f:
    f.write(data)
