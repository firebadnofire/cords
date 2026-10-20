#!/usr/bin/env python3
"""Convert executed GUI observations or Qt XML into the shared JSON/JUnit schema."""
import argparse
import json
from pathlib import Path
import xml.etree.ElementTree as ET

from host import Results


def qt_reports(directory, destination):
    result = Results(destination)
    for name in ('qml-controls', 'native-media', 'native-controller', 'row-filter'):
        path = Path(directory) / (name + '.xml')
        root = ET.parse(path).getroot()
        functions = root.findall('.//TestFunction')
        if not functions:
            raise RuntimeError('Qt result contains no test functions: ' + str(path))
        for function in functions:
            incidents = function.findall('Incident')
            skipped = [m for m in function.findall('Message') + incidents if m.get('type') in ('skip', 'xfail')]
            if any(i.get('type') in ('fail', 'xpass', 'bfail') for i in incidents):
                state = 'fail'
            elif skipped:
                state = 'unavailable'
            elif incidents and all(i.get('type') == 'pass' for i in incidents):
                state = 'pass'
            else:
                state = 'error'
            result.record(name + '/' + function.get('name'), state,
                          dict(source=str(path), messages=[m.findtext('Description') for m in skipped]))
    return result


def gui_report(path, destination):
    value = json.loads(Path(path).read_text())
    if not value.get('passed') or not value.get('driver') or not value.get('package_sha256'):
        raise RuntimeError('GUI observation report lacks executed assertions or package/driver identity')
    for key in ('passed', 'unavailable'):
        if not isinstance(value.get(key), list) or any(not isinstance(n, str) or not n for n in value[key]):
            raise RuntimeError('GUI assertion lists must contain nonempty names')
    if not any(p.is_file() and p.suffix.lower() in ('.jpg', '.jpeg', '.png')
               for p in Path(path).parent.rglob('*')):
        raise RuntimeError('GUI observation report has no screenshot evidence')
    result = Results(destination)
    for state, key in (('pass', 'passed'), ('unavailable', 'unavailable')):
        for name in value.get(key, []):
            result.record('windows-gui/' + name, state,
                          dict(report=str(path), run=value['run'], driver=value['driver'],
                               package_sha256=value['package_sha256']))
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--qt', type=Path)
    group.add_argument('--gui', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = qt_reports(args.qt, args.output) if args.qt else gui_report(args.gui, args.output)
    except Exception as exc:
        Results(args.output).record('aggregation', 'error', exc)
        raise SystemExit(2)
    raise SystemExit(2 if any(c['state'] == 'error' for c in result.cases) else
                     1 if any(c['state'] == 'fail' for c in result.cases) else
                     3 if any(c['state'] == 'unavailable' for c in result.cases) else 0)
