#!/usr/bin/env python3
"""Developer-only X11 interaction check for MusicTerrain's real native editor."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,default=Path('target/debug/musicterrain'))
    p.add_argument('--sysroot',type=Path)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();binary=a.binary.resolve();a.output.mkdir(parents=True,exist_ok=False);out=a.output.resolve()
    env=os.environ.copy();xdotool='xdotool'
    if a.sysroot:
        root=a.sysroot.resolve();env['LD_LIBRARY_PATH']=str(root/'usr/lib/x86_64-linux-gnu')+':'+env.get('LD_LIBRARY_PATH','')
        env['VK_DRIVER_FILES']=str(root/'usr/share/vulkan/icd.d/lvp_icd.json');xdotool=str(root/'usr/bin/xdotool')
    if not env.get('DISPLAY'):p.error('requires X11 display (Xvfb is supported)')
    with tempfile.TemporaryDirectory(prefix='musicterrain-interact-') as scratch:
        scratch=Path(scratch)
        for key in ('HORIZON_HOME','HORIZON_LAUNCHER_HOME','XDG_DATA_HOME','XDG_CONFIG_HOME','XDG_RUNTIME_DIR'):
            folder=scratch/key.lower();folder.mkdir(mode=0o700);env[key]=str(folder)
        env['HORIZON_LAUNCHER_NO_SELF_UPDATE']='1';env['HORIZON_NO_AUDIO']='1';env['RUST_LOG']='musicterrain=info'
        log_path=out/'interaction.log';preset=scratch/'preset.json'
        def text():return log_path.read_text()
        def wait_until(predicate,timeout=15):
            start=time.monotonic()
            while not predicate():
                if process.poll() is not None:raise RuntimeError('demo exited during interactions')
                if time.monotonic()-start>timeout:raise RuntimeError('interaction condition timed out')
                time.sleep(.05)
        def xdo(*args):return subprocess.run([xdotool,*map(str,args)],env=env,check=True,capture_output=True,text=True).stdout.strip()
        with log_path.open('w') as log:
            process=subprocess.Popen([str(binary),'demo','--silent-preview','--preset-file',str(preset)],env=env,stdout=log,stderr=log)
            try:
                wait_until(lambda:len(re.findall('preview mesh uploaded',text()))>=4,30)
                window=xdo('search','--onlyvisible','--name','MusicTerrain').splitlines()[-1]
                xdo('windowfocus','--sync',window)
                def click(x,y):xdo('mousemove','--window',window,x,y,'click','1');time.sleep(.25)
                def key(k):xdo('key','--window',window,k);time.sleep(.2)
                def image(name):subprocess.run(['import','-window',window,str(out/name)],env=env,check=True)
                key('space');time.sleep(.6)
                stable_count=text().count('preview mesh uploaded');time.sleep(.7)
                assert text().count('preview mesh uploaded')==stable_count,'paused terrain should stop rebuilding'
                key('Right');time.sleep(.4)
                click(100,185);time.sleep(.4);image('noise-lab.png')
                click(1180,388);key('ctrl+z');key('ctrl+y')
                click(1120,30);wait_until(preset.exists)
                saved=preset.read_bytes();click(1120,30);assert preset.read_bytes()==saved,'Save must preserve existing file'
                click(1200,30)
                click(100,270);click(1130,215);time.sleep(.5);image('wave-terrain.png')
                click(1130,303);time.sleep(.8);image('floating-islands.png')
                click(100,230);time.sleep(.3);image('mapping-editor.png')
                click(100,360);time.sleep(.3);image('diagnostics.png')
                key('space');time.sleep(.6);key('Escape');process.wait(timeout=10)
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:process.wait(timeout=10)
                    except subprocess.TimeoutExpired:process.kill();process.wait()
        evidence=text()
        for action in ('Page(Noise)','Parameter(Amplitude','Undo','Redo','Save','Load','Preset(1)','Preset(3)','Page(Mapping)','Page(Diagnostics)','Seek('):
            assert action in evidence,f'expected actual UI event: {action}'
        report={'exit_code':process.returncode,'pause_stopped_mesh_updates':True,'save_preserved_existing_preset':True,'events':len(re.findall('editor action',evidence)),'distinct_mesh_hashes':len(set(re.findall(r'mesh_hash=(\d+)',evidence))),'screenshots':['noise-lab.png','wave-terrain.png','floating-islands.png','mapping-editor.png','diagnostics.png'],'clock':'explicit silent diagnostic clock'}
        (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
        if process.returncode:raise SystemExit('interaction demo failed')


if __name__=='__main__':main()
