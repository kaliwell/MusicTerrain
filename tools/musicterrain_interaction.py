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
    p.add_argument('--scene',action='store_true',help='verify real vegetation/water/cave switches')
    p.add_argument('--biomes',action='store_true',help='cycle every biome through the real Settings control')
    p.add_argument('--villages',action='store_true',help='verify actual village showcase/off geometry')
    p.add_argument('--underground',action='store_true',help='verify mine toggles and underground views')
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
                key('space')
                stable_since=time.monotonic();stable_count=text().count('preview mesh uploaded')
                deadline=time.monotonic()+10
                while time.monotonic()-stable_since < .8:
                    count=text().count('preview mesh uploaded')
                    if count!=stable_count:stable_count=count;stable_since=time.monotonic()
                    assert time.monotonic()<deadline,'paused worker did not drain'
                    time.sleep(.05)
                time.sleep(.3)
                assert text().count('preview mesh uploaded')==stable_count,'paused terrain should stop rebuilding'
                if a.scene:
                    def last_scene():
                        records=[line for line in text().splitlines() if 'preview mesh uploaded' in line]
                        return dict((k,int(v)) for k,v in re.findall(r'(trees|plants|cave_faces|water_quads)=(\d+)',records[-1])) if records else {}
                    click(100,315)
                    click(1130,484);wait_until(lambda:last_scene().get('trees')==0 and last_scene().get('plants')==0)
                    click(1130,523);wait_until(lambda:last_scene().get('water_quads')==0)
                    click(1130,561);wait_until(lambda:last_scene().get('cave_faces')==0)
                    image('scene-disabled.png')
                    click(1130,484);click(1130,523);click(1130,561)
                    wait_until(lambda:all(last_scene().get(k,0)>0 for k in ('trees','plants','cave_faces','water_quads')))
                    image('scene-enabled.png')
                if a.biomes:
                    click(100,315)
                    biome_hashes=[]
                    for biome in ('meadow','forest','alpine','shore','birch-forest','taiga','desert','badlands','savanna','swamp','jungle','auto'):
                        before=text().count('preview mesh uploaded')
                        click(1130,601)
                        wait_until(lambda:text().count('preview mesh uploaded')>before)
                        records=[line for line in text().splitlines() if 'preview mesh uploaded' in line]
                        biome_hashes.append(re.search(r'mesh_hash=(\d+)',records[-1]).group(1))
                        image('biome-'+biome+'.png')
                    assert len(set(biome_hashes))>=9,'biome controls did not produce distinct actual geometry'
                if a.villages:
                    click(100,315)
                    def houses():
                        records=[line for line in text().splitlines() if 'preview mesh uploaded' in line]
                        match=re.search(r'houses=(\d+)',records[-1]) if records else None
                        return int(match.group(1)) if match else -1
                    click(1130,640);wait_until(lambda:houses()>=2)
                    image('village-showcase.png')
                    click(1130,640);wait_until(lambda:houses()==0)
                    image('village-off.png')
                    click(1130,640)
                if a.underground:
                    click(100,405)
                    def rails():
                        records=[line for line in text().splitlines() if 'preview mesh uploaded' in line]
                        match=re.search(r'rails=(\d+)',records[-1]) if records else None
                        return int(match.group(1)) if match else -1
                    before=text().count('preview mesh uploaded');click(1130,220)
                    wait_until(lambda:text().count('preview mesh uploaded')>before and rails()>0)
                    for name in ('cutaway','underground'):
                        before=text().count('preview mesh uploaded');click(1130,265)
                        wait_until(lambda:text().count('preview mesh uploaded')>before and rails()>0)
                        image('mine-'+name+'.png')
                    click(1130,220);wait_until(lambda:rails()==0)
                    image('mine-off.png')
                    click(1130,220);click(1130,265)
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
        report={'underground_switches_verified':a.underground,'village_switches_verified':a.villages,'biome_switches_verified':a.biomes,'scene_switches_verified':a.scene,'exit_code':process.returncode,'pause_stopped_mesh_updates':True,'save_preserved_existing_preset':True,'events':len(re.findall('editor action',evidence)),'distinct_mesh_hashes':len(set(re.findall(r'mesh_hash=(\d+)',evidence))),'screenshots':['noise-lab.png','wave-terrain.png','floating-islands.png','mapping-editor.png','diagnostics.png'],'clock':'explicit silent diagnostic clock'}
        (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
        if process.returncode:raise SystemExit('interaction demo failed')


if __name__=='__main__':main()
