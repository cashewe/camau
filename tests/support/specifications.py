def task_spec(task="echo"):
    return {
        "entry": "call",
        "output": "call",
        "nodes": [{"id": "call", "type": "task", "task": task}],
    }


def gated_spec():
    return {
        "entry": "gate",
        "output": "mapped",
        "nodes": [
            {
                "id": "gate",
                "type": "deterministic-gate",
                "select": "/score",
                "cases": [
                    {"operator": "in_range", "lower": 0, "upper": 10, "target": "supported"},
                    {"operator": "otherwise", "target": "unsupported"},
                ],
            },
            {"id": "supported", "type": "task", "task": "supported", "next": "mapped"},
            {
                "id": "mapped",
                "type": "map-schema",
                "mappings": [
                    {"target": "/score", "path-to-source": "/score", "type": "number"},
                    {"target": "/model", "default": "supported", "type": "string"},
                ],
            },
            {"id": "unsupported", "type": "raise-error", "message": "unsupported score"},
        ],
    }


def fanout_spec():
    return {
        "entry": "fan",
        "output": "join",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "first", "target": "left"},
                    {"id": "second", "target": "right"},
                ],
            },
            {"id": "left", "type": "task", "task": "left", "next": "join"},
            {"id": "right", "type": "task", "task": "right", "next": "join"},
            {"id": "join", "type": "converge", "inputs": {"first": "left", "second": "right"}},
        ],
    }
