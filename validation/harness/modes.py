"""Execution modes and configuration for test harness."""

import tempfile
from dataclasses import dataclass
from enum import Enum, auto
from pathlib import Path
from typing import Optional


class ExecutionMode(Enum):
    """Test execution modes."""

    YO = auto()  # Direct yo binary
    NATIVE = auto()  # Harness starts gateway + agent processes
    DOCKER = auto()  # Harness starts via docker-compose


@dataclass
class ModeConfig:
    """Configuration for a specific execution mode."""

    mode: ExecutionMode
    project_root: Path
    binary_path: Path  # always target/release/yo
    gateway_url: Optional[str] = None  # ws://localhost:18789/ws for native/docker

    @classmethod
    def for_mode(cls, mode: ExecutionMode, project_root: Path) -> "ModeConfig":
        """Create configuration for a specific mode."""
        binary_path = project_root / "target" / "release" / "yo"

        if mode == ExecutionMode.YO:
            return cls(
                mode=mode,
                project_root=project_root,
                binary_path=binary_path,
                gateway_url=None,
            )
        else:
            # Native and Docker both use gateway
            return cls(
                mode=mode,
                project_root=project_root,
                binary_path=binary_path,
                gateway_url="ws://localhost:18789/ws",
            )

    @property
    def fixtures_dir(self) -> Path:
        """Return path to fixtures directory."""
        return self.project_root / "fixtures"

    @property
    def scratch_dir(self) -> Path:
        """Return path to scratch directory."""
        return Path(tempfile.gettempdir()) / "brainpro-test-scratch"

    @property
    def mock_webapp_dir(self) -> Path:
        """Return path to mock_webapp fixture."""
        return self.fixtures_dir / "mock_webapp"

    @property
    def mock_webapp_scratch(self) -> Path:
        """Return path to mock_webapp scratch copy (under project root for relative path support)."""
        return self.project_root / "fixtures" / "mock_webapp_scratch"

    @property
    def results_dir(self) -> Path:
        """Return path to results directory."""
        return self.project_root / "validation" / "results"
