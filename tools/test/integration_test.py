import os
import signal
import subprocess
import time
from queue import Queue
from threading import Thread

import psutil
import pytest
import requests

PREFIX = "CONTILE_TEST_"
# GLOBALS
SERVER = None
STRICT_LOG_COUNTS = True
HERE_DIR = os.path.abspath(os.path.dirname(__file__) + "/..")
ROOT_DIR = os.path.dirname(HERE_DIR)
OUT_QUEUES = []


def get_settings():
    return dict(
        test_url=os.environ.get("CONTILE_TEST_URL", "http://localhost:8000"),
        server=os.environ.get(
            "CONTILE_TEST_SERVER", "../../target/debug/contile"
        ),
        noserver=os.environ.get("CONTILE_TEST_NOSERVER", None),
    )


def get_rust_binary_path(binary):
    global STRICT_LOG_COUNTS

    rust_bin = ROOT_DIR + "/target/debug/{}".format(binary)
    possible_paths = [
        "/target/debug/{}".format(binary),
        "/{0}/target/release/{0}".format(binary),
        "/{0}/target/debug/{0}".format(binary),
    ]
    while possible_paths and not os.path.exists(rust_bin):  # pragma: nocover
        rust_bin = ROOT_DIR + possible_paths.pop(0)

    if "release" not in rust_bin:
        # disable checks for chatty debug mode binaries
        STRICT_LOG_COUNTS = False
    return rust_bin


def enqueue_output(out, queue):
    for line in iter(out.readline, b""):
        queue.put(line)
    out.close()


def capture_output_to_queue(output_stream):
    log_queue = Queue()
    t = Thread(target=enqueue_output, args=(output_stream, log_queue))
    t.daemon = True
    t.start()
    return log_queue


def setup_server():
    global SERVER
    settings = get_settings()
    if settings.get("noserver"):
        print("using existing server...")
        return
    # Always set test mode
    os.environ.setdefault("CONTILE_TEST_MODE", "True")
    os.environ.setdefault("RUST_LOG", "trace")
    os.environ.setdefault(
        "CONTILE_ADM_SETTINGS", "{}/adm_settings_test.json".format(ROOT_DIR)
    )
    os.environ.setdefault(
        "CONTILE_TEST_FILE_PATH", "{}/tools/test/test_data/".format(ROOT_DIR)
    )

    cmd = [get_rust_binary_path("contile")]
    print("Starting server: {cmd}".format(cmd=cmd))
    SERVER = subprocess.Popen(
        cmd,
        shell=True,
        env=os.environ,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        universal_newlines=True,
    )
    if SERVER.poll():
        print("Could not start server")
        exit(-1)
    OUT_QUEUES.extend(
        [
            capture_output_to_queue(SERVER.stdout),
            capture_output_to_queue(SERVER.stderr),
        ]
    )


def kill_process(process):
    if process:
        proc = psutil.Process(pid=process.pid)
        child_proc = proc.children(recursive=True)
        for p in [proc] + child_proc:
            os.kill(p.pid, signal.SIGTERM)
        process.wait()


@pytest.fixture
def settings():
    return get_settings()


def default_headers(test: str = "default"):
    return {
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:90.0) "
            "Gecko/20100101 Firefox/90.0"
        ),
        "X-Client-Geo-Location": "US,AK",
        "Remote-Addr": "44.236.48.31",
        "Fake-Response": test,
    }


def setup_module():
    settings = get_settings()
    setup_server()
    try_count = 0
    while True:
        try:
            ping = requests.get(
                "{root}/__heartbeat__".format(root=settings.get("test_url"))
            )
            if ping.status_code == 200:
                print("Found server... {root}", settings.get("test_url"))
                break
        except requests.exceptions.ConnectionError:
            pass
        print(".", end="")
        try_count = try_count + 1
        if try_count > 10:
            print("Could not start server")
            exit(-1)
        time.sleep(1)
    return


def teardown_module():
    kill_process(SERVER)
    return


class TestAdm:
    def test_success(self, settings):
        url = "{root}/v1/tiles".format(root=settings.get("test_url"))
        resp = requests.get(url, headers=default_headers())
        assert resp.status_code == 200, "Failed to return"
        reply = resp.json()
        # the default tab list
        # the default "max_tiles" is 2
        tiles = reply.get("tiles")
        assert len(tiles) == 2
        names = map(lambda tile: tile.get("name").lower(), tiles)
        if not settings.get("noserver"):
            assert list(names) == ["acme", "dunder mifflin"]

    def test_bad_adv_host(self, settings):
        if settings.get("noserver"):
            pytest.skip()
            return
        url = "{root}/v1/tiles".format(root=settings.get("test_url"))
        headers = default_headers("bad_adv")
        resp = requests.get(url, headers=headers)
        assert resp.status_code == 200, "Failed to return"
        reply = resp.json()
        tiles = reply.get("tiles")
        assert len(tiles) == 2
        names = map(lambda tile: tile.get("name").lower(), tiles)
        assert list(names) == ["acme", "los pollos hermanos"]

    def test_bad_click_host(self, settings):
        if settings.get("noserver"):
            pytest.skip()
            return
        url = "{root}/v1/tiles".format(root=settings.get("test_url"))
        headers = default_headers("bad_click")
        resp = requests.get(url, headers=headers)
        assert resp.status_code == 200, "Failed to return"
        reply = resp.json()
        tiles = reply.get("tiles")
        assert len(tiles) == 2
        names = map(lambda tile: tile.get("name").lower(), tiles)
        assert list(names) == ["acme", "dunder mifflin"]
