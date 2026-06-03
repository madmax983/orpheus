import sys

def patch_file():
    with open('crates/orpheus-lang/src/types/infer.rs', 'r') as f:
        content = f.read()

    target = """        let saved_env = self.env.clone();
        let result = (|| {
            let mut param_types = Vec::with_capacity(params.len());
            for param in params {
                let ty = self.fresh_var_type();
                self.env
                    .insert(param.clone(), TypeScheme::monomorphic(ty.clone()));
                param_types.push(ty);
            }

            let body_ty = self.infer_expr(expr)?;
            Ok(Type::curried(
                param_types
                    .into_iter()
                    .map(|ty| self.resolve(ty))
                    .collect::<Vec<_>>(),
                self.resolve(body_ty),
            ))
        })();
        self.env = saved_env;
        result"""

    replacement = """        let mut param_types = Vec::with_capacity(params.len());
        let mut previous_vars = Vec::with_capacity(params.len());
        for param in params {
            let ty = self.fresh_var_type();
            let previous = self.env
                .insert(param.clone(), TypeScheme::monomorphic(ty.clone()));
            previous_vars.push((param.clone(), previous));
            param_types.push(ty);
        }

        let body_result = self.infer_expr(expr);

        // Restore environment
        for (param, previous) in previous_vars.into_iter().rev() {
            if let Some(prev) = previous {
                self.env.insert(param, prev);
            } else {
                self.env.remove(&param);
            }
        }

        let body_ty = body_result?;
        Ok(Type::curried(
            param_types
                .into_iter()
                .map(|ty| self.resolve(ty))
                .collect::<Vec<_>>(),
            self.resolve(body_ty),
        ))"""

    if target in content:
        with open('crates/orpheus-lang/src/types/infer.rs', 'w') as f:
            f.write(content.replace(target, replacement))
        print("Patched successfully")
    else:
        print("Target not found")

patch_file()
