"""Generate independent official samples for the narrow smooth-rectangle adapter."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess

import intake

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = ROOT / "tests/conformance/proposals/velox84-png-isolation-v1/controls/minimal-rectangle/scene.excalidraw"


def run(*args):
    p = subprocess.run(list(map(str,args)),cwd=ROOT,capture_output=True,text=True,timeout=300)
    intake.require(p.returncode==0,f"{args}\n{p.stderr}")
    return p.stdout


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output",type=Path)
    parser.add_argument("renderer",type=Path)
    args=parser.parse_args()
    args.output=args.output.resolve()
    args.output.relative_to(ROOT)
    image="mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
    run("docker","run","--rm","--network","none","--platform","linux/amd64",
        "--user",f"{os.getuid()}:{os.getgid()}","--env","HOME=/tmp","--env","REFERENCE_ORACLE_CANONICAL=1",
        "--mount",f"type=bind,src={ROOT},dst=/workspace","--workdir","/workspace/tools/reference-oracle",image,
        "bash","-eu","-c","npm run build && node scripts/profile.mjs verify")
    args.output.mkdir()
    template=json.loads(TEMPLATE.read_bytes())
    cases=[]
    for width in (1.5,2,3,4):
        for phase in (0.125,0.632993081447):
            cases.append((f"w{width}-p{phase}",width,phase,0,100,1))
    cases += [("negative",2,-0.125,0,100,1),("scaled",2,0.632993081447,0,100,1.5),
              ("opacity",2,0.632993081447,0,50,1),("rotated",2,0.632993081447,0.25,100,1)]
    results=[]
    for name,width,phase,angle,opacity,scale in cases:
        directory=args.output / name
        directory.mkdir()
        scene=copy.deepcopy(template)
        anchor,rect=scene["elements"]
        anchor.update(x=-10,y=-10,width=120,height=100,strokeWidth=1)
        rect.update(x=20.233470223713,y=20+phase,width=48,height=36,strokeWidth=width,angle=angle,opacity=opacity)
        plan={"schema":"excalidraw-render/representative-intake-plan/v1","case_id":"diagnostic.png-edges",
              "formats":["svg","png","pdf"],"variants":[{"id":"light","color_mode":"light","include_background":True,"padding":0,"scale":scale,"metadata":False}]}
        (directory / "scene.excalidraw").write_bytes(intake.encode(scene))
        (directory / "plan.json").write_bytes(intake.encode(plan))
        run("docker","run","--rm","--init","--ipc=host","--network","none","--platform","linux/amd64",
            "--user",f"{os.getuid()}:{os.getgid()}","--env","HOME=/tmp","--env","REFERENCE_ORACLE_CANONICAL=1",
            "--mount",f"type=bind,src={ROOT},dst=/workspace","--workdir","/workspace/tools/reference-oracle",image,
            "node","../representative-intake/reference.mjs",directory.relative_to(ROOT), (directory / "reference").relative_to(ROOT))
        ref=json.loads((directory / "reference/reference.json").read_bytes())["rows"][0]
        observed=[]
        for index in range(2):
            output=directory / f"native-{index}.png"
            report=directory / f"report-{index}.json"
            run(args.renderer.resolve(),directory / "scene.excalidraw","--output",output,"--report-output",report,*intake.request_args(plan["variants"][0],"png"))
            intake.validate_report(output.read_bytes(),report.read_bytes(),"png")
            observed.append((output.read_bytes(),report.read_bytes()))
        intake.require(observed[0]==observed[1],"native repeat mismatch")
        region_plan={"width":ref["width"],"height":ref["height"],"whole_class":"geometry/image","crops":[]}
        (directory / "regions.json").write_bytes(intake.encode(region_plan))
        run("cargo","run","--locked","--release","--manifest-path","tools/representative-intake/compare/Cargo.toml","--",
            directory / "reference/light/official-png.rgba.zlib",directory / "native-0.png","canonical",directory / "regions.json",directory / "comparison.json")
        result=json.loads((directory / "comparison.json").read_bytes())
        results.append({"id":name,"result":result})
        print(json.dumps({"id":name,"passed":result["passed"],"whole":result["whole"]}),flush=True)
    (args.output / "qualification.json").write_bytes(intake.encode({"schema":"excalidraw-render/png-edge-qualification/v1",
        "approval":None,"results":results,"renderer":intake.descriptor(args.renderer.read_bytes()),
        "files":{str(p.relative_to(args.output)):intake.descriptor(p.read_bytes()) for p in sorted(args.output.rglob("*")) if p.is_file()}}))


if __name__=="__main__":main()
