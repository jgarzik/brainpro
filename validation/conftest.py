"""Pytest configuration and fixtures for brainpro validation tests."""

import os
import shutil
from pathlib import Path
from typing import Generator, Optional

import pytest

from harness.modes import ExecutionMode, ModeConfig
from harness.runner import BrainproRunner
from harness.fixtures import ScratchDir, MockWebapp, SessionsDir
from harness.native_gateway import NativeGateway
from harness.docker_gateway import DockerGateway


def pytest_addoption(parser):
    """Add custom command line options."""
    parser.addoption(
        "--mode",
        action="store",
        default=None,
        choices=["yo", "native", "docker"],
        help="Execution mode: yo (direct), native (local gateway), docker",
    )


@pytest.fixture(scope="session")
def project_root() -> Path:
    """Return the project root directory."""
    # validation/ is one level below project root
    return Path(__file__).parent.parent


@pytest.fixture(scope="session")
def execution_mode(request) -> ExecutionMode:
    """Determine the execution mode from CLI or environment."""
    mode_str = request.config.getoption("--mode")
    if mode_str is None:
        mode_str = os.environ.get("BRAINPRO_TEST_MODE", "yo")

    return ExecutionMode[mode_str.upper()]


@pytest.fixture(scope="session")
def mode_config(execution_mode: ExecutionMode, project_root: Path) -> ModeConfig:
    """Create configuration for the current mode."""
    return ModeConfig.for_mode(execution_mode, project_root)


@pytest.fixture(scope="session")
def gateway_manager(
    execution_mode: ExecutionMode, project_root: Path
) -> Generator[Optional[str], None, None]:
    """
    Manage gateway lifecycle for native/docker modes.

    Yields the gateway WebSocket URL, or None for yo mode.
    """
    if execution_mode == ExecutionMode.YO:
        yield None
        return

    if execution_mode == ExecutionMode.NATIVE:
        gateway = NativeGateway(project_root)
    else:  # DOCKER
        gateway = DockerGateway(project_root)

    try:
        url = gateway.start()
        yield url
    finally:
        gateway.stop()


@pytest.fixture(scope="session")
def session_mode_config(
    mode_config: ModeConfig, gateway_manager: Optional[str]
) -> ModeConfig:
    """
    Return mode config updated with actual gateway URL if applicable.

    This is the config to use for creating runners.
    """
    if gateway_manager is not None:
        # Update the config with the actual gateway URL
        return ModeConfig(
            mode=mode_config.mode,
            project_root=mode_config.project_root,
            binary_path=mode_config.binary_path,
            gateway_url=gateway_manager,
        )
    return mode_config


@pytest.fixture
def runner(session_mode_config: ModeConfig) -> BrainproRunner:
    """Create a BrainproRunner for tests."""
    return BrainproRunner(session_mode_config)


@pytest.fixture
def scratch_dir(mode_config: ModeConfig) -> Generator[ScratchDir, None, None]:
    """
    Provide a clean scratch directory.

    Resets before yielding, does not cleanup after (for debugging).
    """
    scratch = ScratchDir(mode_config)
    scratch.reset()
    yield scratch


@pytest.fixture
def mock_webapp(mode_config: ModeConfig) -> Generator[MockWebapp, None, None]:
    """
    Provide a fresh mock_webapp scratch copy.

    Resets before yielding, cleans up after test.
    """
    webapp = MockWebapp(mode_config)
    webapp.reset()
    yield webapp
    webapp.cleanup()


@pytest.fixture
def webapp_runner(session_mode_config: ModeConfig, mock_webapp: MockWebapp) -> BrainproRunner:
    """Create a runner for mock_webapp tests (runs from project root)."""
    # Note: Runs from project root so yo can find its config.
    # Tests should use absolute paths like "/tmp/brainpro-mock-webapp-scratch/src/..."
    return BrainproRunner(session_mode_config)


@pytest.fixture
def mock_webapp_path(mock_webapp: MockWebapp, mode_config: ModeConfig) -> str:
    """Return the relative path to mock_webapp_scratch from project root."""
    return str(mock_webapp.path.relative_to(mode_config.project_root))


@pytest.fixture
def sessions_dir() -> Generator[SessionsDir, None, None]:
    """Provide a clean sessions directory."""
    sessions = SessionsDir()
    sessions.reset()
    yield sessions


# =============================================================================
# Markers
# =============================================================================


def pytest_configure(config):
    """Register custom markers."""
    config.addinivalue_line(
        "markers", "gateway_only: mark test as requiring gateway mode"
    )


def pytest_collection_modifyitems(config, items):
    """Skip gateway_only tests in yo mode."""
    mode_str = config.getoption("--mode")
    if mode_str is None:
        mode_str = os.environ.get("BRAINPRO_TEST_MODE", "yo")

    if mode_str.lower() == "yo":
        skip_gateway = pytest.mark.skip(reason="Requires gateway mode (native/docker)")
        for item in items:
            if "gateway_only" in item.keywords:
                item.add_marker(skip_gateway)


# =============================================================================
# Helpers
# =============================================================================


@pytest.fixture
def fixtures_dir(mode_config: ModeConfig) -> Path:
    """Return the fixtures directory path.
    
    Also ensures fixtures/scratch exists and is clean for each test,
    and restores hello_repo/src/lib.rs to its canonical state (in case
    prior tests accidentally modified it).
    """
    scratch = mode_config.fixtures_dir / "scratch"
    if scratch.exists():
        shutil.rmtree(scratch, ignore_errors=True)
    scratch.mkdir(parents=True, exist_ok=True)
    
    # Restore hello_repo/src/lib.rs to canonical state
    lib_rs = mode_config.fixtures_dir / "hello_repo" / "src" / "lib.rs"
    canonical_content = '''pub fn greet(name: &str) -> String {
    // TODO: add proper greeting
    format!("Hello, {}!", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("World"), "Hello, World!");
    }
}
'''
    lib_rs.write_text(canonical_content)
    
    return mode_config.fixtures_dir


@pytest.fixture
def hello_repo(fixtures_dir: Path) -> Path:
    """Return the hello_repo fixture path."""
    return fixtures_dir / "hello_repo"
