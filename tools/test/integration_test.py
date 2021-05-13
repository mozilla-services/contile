import os;
import sys;
import configargparse;
import pytest;
import requests;

PREFIX = "CONTILE_TEST_";

def settings():
    parser = configargparse.ArgParser(
        default_config_files=["test_settings.ini"],
        ignore_unknown_config_file_keys=True)
    parser.add("-c", "--config", is_config_file=True, help="Config file path")
    parser.add("-u", "--test_url", help="Target test base URL", default="http://localhost:8000")
    parser.add("--server", help="path to the test server", default="../../target/debug/contile")
    return parser.parse_known_args()[0]

def default_headers(test:str = "default"):
    return {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:90.0) Gecko/20100101 Firefox/90.0",
        "Remote-Addr": "44.236.48.31",
        "Fake-Response": test,
    }

def test_success():
    conf = settings()
    url = "{root}/v1/tiles?country={country}&placement={placement}".format(
            root=conf.test_url,
            country="US",
            placement="newtab")
    resp = requests.get(url,
        headers=default_headers())
    assert resp.status_code == 200, "Failed to return"
    import pdb; pdb.set_trace()
    reply = resp.json()
    assert len(reply.get("tiles")) == 3
