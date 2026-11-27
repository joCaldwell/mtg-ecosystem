"""Save trainable state and reject incompatible environment identities."""

from dataclasses import asdict
from pathlib import Path
from tempfile import NamedTemporaryFile

import torch

from .schema import Identity


def save_checkpoint(path: Path, model, optimizer, identity: Identity) -> None:
    payload = {
        "identity": asdict(identity),
        "model": model.state_dict(),
        "optimizer": optimizer.state_dict(),
    }
    with NamedTemporaryFile(dir=path.parent, delete=False) as file:
        temporary = Path(file.name)
    try:
        torch.save(payload, temporary)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def load_checkpoint(path: Path, model, identity: Identity, optimizer=None) -> None:
    payload = torch.load(path, map_location="cpu", weights_only=True)
    expected = asdict(identity)
    actual = payload.get("identity")
    if actual != expected:
        differences = (
            ", ".join(key for key, value in expected.items() if actual.get(key) != value)
            if isinstance(actual, dict) else "missing identity"
        )
        raise ValueError(f"Checkpoint environment identity mismatch ({differences or 'extra fields'})")
    model.load_state_dict(payload["model"])
    if optimizer is not None:
        optimizer.load_state_dict(payload["optimizer"])
