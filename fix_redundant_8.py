import re

with open('crates/orpheus-lang/src/types/infer.rs', 'r') as f:
    content = f.read()

content = content.replace('self.unify(&actual.clone(), &Type::pattern(Type::Number))', 'self.unify(&actual, &Type::pattern(Type::Number))')
content = content.replace('self.unify(&ty.clone(), &Type::pattern(element))?', 'self.unify(&ty, &Type::pattern(element))?')
content = content.replace('self.unify(&expected.clone(), actual.clone())', 'self.unify(&expected, &actual)')

# fix any remaining &var.clone()
content = re.sub(r'&([a-zA-Z_0-9]+)\.clone\(\)', r'&\1', content)
content = re.sub(r'&Type::pattern\(([^)]+)\.clone\(\)\)', r'&Type::pattern(\1)', content)

with open('crates/orpheus-lang/src/types/infer.rs', 'w') as f:
    f.write(content)
