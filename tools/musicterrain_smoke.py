#!/usr/bin/env python3
"""Developer harness: real Horizon-rendered MusicTerrain screenshot and RSS evidence."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/debug/musicterrain'))
    parser.add_argument('--sysroot', type=Path)
    parser.add_argument('--output', type=Path, required=True, help='new evidence directory')
    parser.add_argument('--frames', type=int, default=30)
    parser.add_argument('--size', type=int, default=64, choices=(64,128,256))
    parser.add_argument('--audio', type=Path)
    parser.add_argument('--resources', type=Path)
    parser.add_argument('--require-scene', action='store_true')
    parser.add_argument('--biome', default='auto')
    parser.add_argument('--villages', choices=('off','auto','showcase'), default='auto')
    parser.add_argument('--require-village', action='store_true')
    for feature in ('vegetation','water','caves'):
        parser.add_argument('--no-'+feature, action='store_true')
    args=parser.parse_args()
    binary=args.binary.resolve()
    if not binary.is_file():
        parser.error('build the musicterrain native binary first')
    args.output.mkdir(parents=True, exist_ok=False)
    output=args.output.resolve()
    env=os.environ.copy()
    if args.sysroot:
        root=args.sysroot.resolve()
        env['LD_LIBRARY_PATH']=str(root/'usr/lib/x86_64-linux-gnu')+':'+env.get('LD_LIBRARY_PATH','')
        driver=root/'usr/share/vulkan/icd.d/lvp_icd.json'
        if driver.exists():
            env['VK_DRIVER_FILES']=str(driver)
    if not env.get('DISPLAY') and not env.get('WAYLAND_DISPLAY'):
        parser.error('a display is required; start Xvfb or run on a desktop')
    with tempfile.TemporaryDirectory(prefix='musicterrain-render-') as scratch:
        scratch=Path(scratch)
        for key in ('HORIZON_HOME','HORIZON_LAUNCHER_HOME','XDG_DATA_HOME','XDG_CONFIG_HOME','XDG_RUNTIME_DIR'):
            folder=scratch/key.lower();folder.mkdir(mode=0o700)
            env[key]=str(folder)
        env['HORIZON_LAUNCHER_NO_SELF_UPDATE']='1'
        env['HORIZON_NO_AUDIO']='1'
        env['RUST_LOG']='musicterrain=info'
        command=[str(binary),'demo','--silent-preview','--offscreen','--frames',str(args.frames),'--size',str(args.size),'--screenshot',str(output/'preview.png'),'--preset-file',str(scratch/'preset.json'),'--log-dir',str(output/'logs')]
        command.extend(['--biome',args.biome,'--villages',args.villages])
        if args.audio:
            command.extend(['--audio',str(args.audio.resolve())])
        if args.resources:
            command.extend(['--resources',str(args.resources.resolve())])
        for feature in ('vegetation','water','caves'):
            if getattr(args,'no_'+feature):command.append('--no-'+feature)
        started=time.monotonic();rss=0
        with (output/'stderr.log').open('w') as log:
            process=subprocess.Popen(command,env=env,stdout=log,stderr=log)
            try:
                while process.poll() is None:
                    status=Path(f'/proc/{process.pid}/status')
                    if status.exists():
                        match=re.search(r'VmHWM:\s+(\d+)',status.read_text())
                        if match:rss=max(rss,int(match.group(1)))
                    if time.monotonic()-started>120:
                        process.terminate()
                        try:process.wait(timeout=10)
                        except subprocess.TimeoutExpired:process.kill();process.wait()
                        raise RuntimeError('renderer smoke exceeded120s')
                    time.sleep(0.05)
            finally:
                if process.poll() is None:process.terminate();process.wait(timeout=10)
        elapsed=time.monotonic()-started
        text=(output/'stderr.log').read_text()
        hashes=re.findall(r'mesh_hash=(\d+)',text)
        upload_records=[line for line in text.splitlines() if 'preview mesh uploaded' in line]
        meshes=[float(v) for line in upload_records for v in re.findall(r'generation_ms=([\d.]+)',line)]
        scenes=[dict((key,int(value)) for key,value in re.findall(r'(villages|houses|farms|trees|plants|cave_faces|water_quads|material_kinds)=(\d+)',line)) for line in upload_records]
        scenes=[scene for scene in scenes if scene]
        completion=re.search(r'frames=(\d+) meshes=(\d+) distinct_meshes=(\d+) elapsed=([\d.]+)',text)
        performance=[dict((key,float(value)) for key,value in re.findall(r'(frame_ms_mean|frame_ms_max|fps|queued_jobs|generation_ms|upload_ms)=([\d.]+)',line)) for line in text.splitlines() if 'preview performance' in line]
        report={'scene_samples':scenes,'performance_samples':performance,'exit_code':process.returncode,'wall_seconds':elapsed,'peak_rss_kib_sampled':rss,'distinct_mesh_hashes':len(set(hashes)),'mesh_updates':len(hashes),'mesh_mean_ms':sum(meshes)/len(meshes) if meshes else None,'mesh_max_ms':max(meshes) if meshes else None,'extent':args.size,'resolution':[1280,800],'audio':'imported file' if args.audio else 'synthesized12s','clock':'explicit silent software clock','graphics':'software Vulkan requested' if args.sysroot else 'environment adapter','build_binary':str(binary),'screenshot':'preview.png','completion_log':completion.group(0) if completion else None}
        (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report,indent=2))
        if args.require_village:
            assert any(scene.get('houses',0)>=2 and scene.get('farms',0)>0 for scene in scenes),'missing actual village geometry'
        if args.require_scene:
            assert scenes,'missing scene geometry statistics'
            for feature,key in [('vegetation','trees'),('vegetation','plants'),('water','water_quads'),('caves','cave_faces')]:
                if not getattr(args,'no_'+feature):
                    assert any(scene.get(key,0)>0 for scene in scenes),'missing actual scene geometry: '+key
                else:
                    assert all(scene.get(key,0)==0 for scene in scenes),'disabled scene geometry remained: '+key
        if process.returncode or len(set(hashes))<2 or not (output/'preview.png').is_file() or (output/'preview.png').stat().st_size < 1024:
            raise SystemExit('Smoke failed; inspect stderr.log')


if __name__=='__main__':main()
