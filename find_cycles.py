import re
import os
from collections import defaultdict

def parse_imports(file_path):
    imports = set()
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            for line in f:
                # very simple matching for `use crate::foo::bar`
                match = re.search(r'use\s+crate::([a-zA-Z0-9_]+)', line)
                if match:
                    imports.add(match.group(1))
                match = re.search(r'use\s+super::([a-zA-Z0-9_]+)', line)
                if match:
                    # just skip super for now or try to resolve
                    pass
    except:
        pass
    return imports

def build_graph(src_dir):
    graph = defaultdict(set)
    for root, dirs, files in os.walk(src_dir):
        for file in files:
            if file.endswith('.rs'):
                path = os.path.join(root, file)
                mod_name = os.path.splitext(file)[0]
                if mod_name == 'mod':
                    mod_name = os.path.basename(root)
                imports = parse_imports(path)
                for imp in imports:
                    if imp != mod_name:
                        graph[mod_name].add(imp)
    return graph

def find_cycles(graph):
    visited = set()
    path = []
    cycles = []

    def dfs(node):
        if node in path:
            cycle = path[path.index(node):] + [node]
            cycles.append(cycle)
            return
        if node in visited:
            return
        visited.add(node)
        path.append(node)
        for neighbor in graph.get(node, []):
            dfs(neighbor)
        path.pop()

    for node in graph:
        dfs(node)

    return cycles

dsp_graph = build_graph('crates/orpheus-dsp/src')
lang_graph = build_graph('crates/orpheus-lang/src')

print("DSP Cycles:")
for c in find_cycles(dsp_graph):
    print(" -> ".join(c))

print("Lang Cycles:")
for c in find_cycles(lang_graph):
    print(" -> ".join(c))
