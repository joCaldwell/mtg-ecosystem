"""Exercise the extension boundary on two independent thread schedules."""
import ast
import sys
from pathlib import Path
import numpy as np
from qualify import actions, load

mod = load(Path(sys.argv[1]))
n = 8
envs = [mod.VecEnv("commander-foundations-v1", None, n, threads, 86) for threads in (1, 8)]
fields = dict(globals=(48,), players=(4, 24), ent_int=(256, 10), ent_num=(256, 24),
              ent_mask=(256,), cand=(128, 6), ptr_mask=(131,), num_mask=(64,),
              head=(), seat=(), reward=(4,), done=(), info=(8,))
# mtg_env.pyi names exactly the public classes, methods and Batch keys.
stub = ast.parse(Path(__file__).with_name("mtg_env.pyi").read_text())
classes = {c.name: c for c in stub.body if isinstance(c, ast.ClassDef) and c.name[0] != "_"}
assert classes.keys() == {k for k in vars(mod) if k[0] != "_"}, sorted(classes)
for name, node in classes.items():
    methods = {f.name for f in node.body if isinstance(f, ast.FunctionDef)}
    assert all(hasattr(getattr(mod, name), m) for m in methods), name
    assert {m for m in methods if m[0] != "_"} == {m for m in dir(getattr(mod, name)) if m[0] != "_"}, name
keys = {e.value for f in classes["Batch"].body if isinstance(f, ast.FunctionDef) and f.name == "__getitem__"
        for e in ast.walk(f.args) if isinstance(e, ast.Constant)}
assert keys == fields.keys(), keys ^ fields.keys()
rng = np.random.default_rng(2)
for env in envs:
    env.reset()
catalog = envs[0].catalog()
assert catalog["card_len"].shape == (len(catalog["card_tokens"]),)
assert catalog["card_tokens"].shape[1] == 192 and catalog["card_num"].shape[1] == 16
with envs[0].buffers() as batch:
    retained = batch["ent_int"][:, :1]
    address = batch["ent_int"].ctypes.data
for operation in (envs[0].reset, lambda: envs[0].step(np.zeros(n, dtype=np.int32))):
    try:
        operation()
    except RuntimeError as error:
        assert "leased" in str(error)
    else:
        raise AssertionError("derived ndarray lost its lease")
assert not retained.flags.writeable
del retained
for step in range(256):
    with envs[0].buffers() as a, envs[1].buffers() as b:
        for k, shape in fields.items():
            assert a[k].shape == (n, *shape)
            assert a[k].dtype == (np.uint8 if k in ("ent_mask","ptr_mask","num_mask","head","seat","done") else np.int32)
            assert a[k].flags.c_contiguous and not a[k].flags.writeable
            assert a[k].tobytes() == b[k].tobytes(), (k, step)
        assert a["ent_int"].ctypes.data == address
        chosen = actions(a, rng)
    for env in envs:
        env.step(chosen)
assert envs[0].state_digests() == envs[1].state_digests()
print(f"Python {sys.version.split()[0]}, NumPy {np.__version__}: all 13 buffers, 256 steps x {n} games, digests, leases and address reuse passed")
