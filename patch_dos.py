import re

with open('tests/test_parse_dos.rs', 'r') as f:
    content = f.read()

content = content.replace('let msg = format!("{:?}", e);', 'let msg = format!("{e:?}");')
content = content.replace('payload.push_str("1");', "payload.push('1');")

pattern = r'            if !msg\.contains\("maximum AST depth exceeded"\) \{\n                panic!\("Expected max AST depth error, got: \{\}", msg\);\n            \}'
replacement = r'            assert!(msg.contains("maximum AST depth exceeded"), "Expected max AST depth error, got: {msg}");'
content = re.sub(pattern, replacement, content)

with open('tests/test_parse_dos.rs', 'w') as f:
    f.write(content)
