// Browser controls for the same versioned presets, parameters and mapping graph as Rust.
(function(){
  'use strict';
  const $=id=>document.getElementById(id),api=()=>window.MusicTerrain;
  let meta,originalAtlas,undo=[],redo=[],tab='scene',objectURL=null,context=null,analyser=null,mediaSource=null;
  const names={Frequency:'频率',Scale:'缩放',BaseHeight:'基础高度',Amplitude:'振幅',Persistence:'持续度',Lacunarity:'频率倍增',Gain:'增益',DomainWarp:'域扭曲',DensityThreshold:'密度阈值',CaveThreshold:'洞穴阈值',CaveW:'洞穴 W 维偏移',CaveSpeed:'洞穴演化速度',RidgeStrength:'山脊强度',ErosionInfluence:'侵蚀影响',StructureDensity:'结构密度（预留）',Detail:'细节',IslandStrength:'浮岛强度',ColorShift:'颜色偏移',ParticleIntensity:'粒子强度（预留）',Shockwave:'冲击波',EffectIntensity:'效果强度'};
  const snake=name=>name.replace(/[A-Z]/g,(c,i)=>(i?'_':'')+c.toLowerCase());
  const clone=value=>JSON.parse(JSON.stringify(value));
  const current=()=>clone(api().config);
  function change(patch,remember=true){if(remember){undo.push(current());undo=undo.slice(-40);redo=[];}api().edit(patch);}
  function record(){undo.push(current());undo=undo.slice(-40);redo=[];}
  function sync(){for(const control of document.querySelectorAll('[data-scene-property]')){const value=api().config[control.dataset.sceneProperty];if(control.type==='checkbox')control.checked=value;else control.value=value;}}
  function element(tag,text){const el=document.createElement(tag);if(text!==undefined)el.textContent=text;return el;}
  function select(values,value,update){const el=element('select');for(const [key,label] of values){const option=element('option',label);option.value=key;el.append(option);}el.value=String(value);el.onchange=()=>update(el.value);return el;}
  function number(value,min,max,step,update){const el=element('input');el.type='number';el.min=min;el.max=max;el.step=step;el.value=value;el.onchange=()=>{const n=Number(el.value);if(Number.isFinite(n)&&n>=min&&n<=max)update(n);else el.value=value;};return el;}
  function row(parent,title,control){const label=element('label');label.append(element('span',title),control);parent.append(label);return control;}
  function checkbox(value,update){const el=element('input');el.type='checkbox';el.checked=value;el.onchange=()=>update(el.checked);return el;}
  function button(parent,title,action){const el=element('button',title);el.onclick=action;parent.append(el);return el;}
  function report(error){$('editor-message').textContent=String(error?.message||error);}
  function presetObject(){const c=api().config;return {version:1,name:$('preset-name').value.trim()||'网页自定义',seed:c.seed,parameters:{...c.parameters,noise_type:['Perlin','Simplex','Fbm','Ridged','Billow','Cellular','Value','Terraced'][c.noise],cave_speed:c.cavespeed},mappings:c.mappings};}
  function load(preset){
    if(!preset||![0,1].includes(preset.version)||!preset.parameters||!Array.isArray(preset.mappings?.rules)||preset.mappings.rules.length>128||!Number.isSafeInteger(preset.seed))throw new Error('不是兼容的 MusicTerrain 预设');
    if(typeof preset.name!=='string'||!preset.name.trim()||new TextEncoder().encode(preset.name).length>128)throw new Error('预设名称必须为 1 至 128 字节');
    // Validate numbers before sending; Rust remains the authoritative validator.
    for(const target of meta.targets){const v=preset.parameters[snake(target.name)];if(v!==undefined&&(!Number.isFinite(v)||v<target.bounds[0]||v>target.bounds[1]))throw new Error('参数越界：'+target.name);}
    if(!Number.isInteger(preset.parameters.octaves)||preset.parameters.octaves<1||preset.parameters.octaves>8)throw new Error('octaves must be 1..8');
    const noise=['Perlin','Simplex','Fbm','Ridged','Billow','Cellular','Value','Terraced'].indexOf(preset.parameters.noise_type);if(noise<0)throw new Error('未知噪波');
    change({seed:preset.seed,noise,parameters:clone(preset.parameters),mappings:clone(preset.mappings),cavespeed:preset.parameters.cave_speed??.12});$('preset-name').value=preset.name||'网页自定义';renderParameters();renderMappings();report('已载入预设');
  }
  function renderParameters(){const parent=$('parameters');parent.replaceChildren();const c=api().config;
    row(parent,'随机种子',number(c.seed,-2147483648,2147483647,1,v=>change({seed:v})));
    row(parent,'倍频层数',number(c.parameters.octaves,1,8,1,v=>change({parameters:{...api().config.parameters,octaves:v}})));
    for(const target of meta.targets){const key=snake(target.name),[min,max]=target.bounds;
      const input=number(c.parameters[key],min,max,(max-min)<=2?.001:.01,v=>{change({parameters:{...api().config.parameters,[key]:v},...(key==='cave_speed'?{cavespeed:v}:{})});});
      row(parent,names[target.name]||target.name,input);
    }
  }
  const sources=[...Array.from({length:7},(_,i)=>[JSON.stringify({Band:i}),'频段 '+(i+1)]),...['Rms','Centroid','Flux','Onset','BeatStrength','BeatPhase'].map(s=>[JSON.stringify(s),({Rms:'总能量',Centroid:'频谱重心',Flux:'频谱通量',Onset:'起音',BeatStrength:'节拍强度',BeatPhase:'节拍相位'})[s]])];
  function renderMappings(){const parent=$('mappings');parent.replaceChildren();const rules=api().config.mappings.rules;
    rules.forEach((rule,index)=>{
      const detail=element('details'),summary=element('summary',`${String(index+1).padStart(2,'0')} ${names[rule.target]||rule.target} · ${rule.enabled?'启用':'关闭'}`);detail.append(summary);parent.append(detail);
      const update=patch=>{const next=clone(api().config.mappings);next.rules[index]={...next.rules[index],...patch};change({mappings:next});};
      row(detail,'启用',checkbox(rule.enabled,v=>update({enabled:v})));
      row(detail,'音频来源',select(sources,JSON.stringify(rule.source),v=>update({source:JSON.parse(v)})));
      row(detail,'目标参数',select(meta.targets.map(t=>[t.name,names[t.name]]),rule.target,v=>update({target:v})));
      for(const [key,label] of [['input_range','输入'],['output_range','输出']])for(let j=0;j<2;j++)row(detail,label+(j?'上限':'下限'),number(rule[key][j],-1000000,1000000,.01,v=>{const range=[...api().config.mappings.rules[index][key]];range[j]=v;if(key==='input_range'&&range[0]>=range[1]){report('输入下限必须小于上限');return;}update({[key]:range});}));
      for(const [key,label,min,max] of [['gain','增益',-1000,1000],['bias','偏置',-1000,1000],['attack','上升（秒）',0,60],['release','回落（秒）',0,60]])row(detail,label,number(rule[key],min,max,.01,v=>update({[key]:v})));
      row(detail,'响应曲线',select(['Linear','Smoothstep','Logarithmic','Power'].map(v=>[v,({Linear:'线性',Smoothstep:'平滑',Logarithmic:'对数',Power:'幂函数'})[v]]),typeof rule.curve==='string'?rule.curve:'Power',v=>update({curve:v==='Power'?{Power:2}:v})));
      row(detail,'幂指数',number(typeof rule.curve==='object'?rule.curve.Power:2,.1,8,.1,v=>update({curve:{Power:v}})));
      row(detail,'叠加方式',select([['Add','相加'],['Replace','替换'],['Multiply','相乘']],rule.combine,v=>update({combine:v})));
      row(detail,'截断输入',checkbox(rule.clamp,v=>update({clamp:v})));
      const actions=element('div');actions.className='actions';detail.append(actions);
      button(actions,'复制',()=>{const graph=clone(api().config.mappings);if(graph.rules.length>=128)return;graph.rules.splice(index+1,0,clone(graph.rules[index]));change({mappings:graph});renderMappings();});
      button(actions,'删除',()=>{const graph=clone(api().config.mappings);graph.rules.splice(index,1);change({mappings:graph});renderMappings();});
    });
  }
  async function loadTexture(files){
    const data=originalAtlas.slice(),byName=new Map([...files].map(f=>[f.name,f]));let count=0;
    const canvas=document.createElement('canvas');canvas.width=canvas.height=32;const ctx=canvas.getContext('2d',{willReadFrequently:true});ctx.imageSmoothingEnabled=false;
    async function decode(name){const file=byName.get(name);if(file.size>16*1024*1024)throw new Error('纹理文件超过 16 MiB：'+name);const bitmap=await createImageBitmap(file);
      if(bitmap.width>2048||bitmap.height>2048||bitmap.width*bitmap.height>4194304){bitmap.close();throw new Error('纹理尺寸过大：'+name);}
      const side=Math.min(bitmap.width,bitmap.height);if(bitmap.width%side||bitmap.height%side){bitmap.close();throw new Error('纹理必须为正方形或完整动画条：'+name);}
      ctx.clearRect(0,0,32,32);ctx.drawImage(bitmap,0,0,side,side,0,0,32,32);bitmap.close();return ctx.getImageData(0,0,32,32).data;
    }
    for(let layer=0;layer<meta.textures.length;layer++){
      const name=meta.textures[layer].find(n=>byName.has(n));if(!name)continue;const pixels=await decode(name);
      if(layer===1&&byName.has('grass_block_side_overlay.png')){
        const overlay=await decode('grass_block_side_overlay.png'),tint=[112,169,76];
        for(let i=0;i<pixels.length;i+=4){const a=overlay[i+3],b=pixels[i+3],out=a+Math.floor((b*(255-a)+127)/255);
          for(let c=0;c<3;c++){const grass=Math.floor((overlay[i+c]*tint[c]+127)/255),premultiplied=grass*a+Math.floor((pixels[i+c]*b*(255-a)+127)/255);pixels[i+c]=out?Math.floor((premultiplied+Math.floor(out/2))/out):0;}pixels[i+3]=out;}
      }
      if(layer===5)for(let i=3;i<pixels.length;i+=4)pixels[i]=Math.min(160,pixels[i]);
      data.set(pixels,layer*4096);count++;
    }
    api().setAtlas(data);report(`已载入 ${count} / 65 种本地纹理，其他使用原创材质`);
  }
  async function fileSelected(file){
    if(!file)return;if(file.size>512*1024*1024)throw new Error('音乐文件上限为 512 MiB');
    const player=$('music-player');player.pause();if(objectURL)URL.revokeObjectURL(objectURL);objectURL=URL.createObjectURL(file);player.src=objectURL;
    context??=new AudioContext();
    if(!mediaSource){mediaSource=context.createMediaElementSource(player);analyser=context.createAnalyser();analyser.fftSize=4096;mediaSource.connect(analyser);analyser.connect(context.destination);}
    const samples=new Float32Array(4096);api().setFileAudio({sample(){analyser.getFloatTimeDomainData(samples);return {pcm:Array.from(samples),rate:context.sampleRate,time:player.currentTime,paused:player.paused};}});
    await context.resume();report('已选择 '+file.name+'，点击播放器播放');
  }
  function ready(metadata,atlas){
    meta=metadata;originalAtlas=atlas.slice();const preset=meta.presets[0];api().edit({parameters:clone(preset.parameters),mappings:clone(preset.mappings)});
    const panel=$('panel'),scene=element('section');scene.id='scene-tab';
    for(const child of [...panel.children])if(!['HEADER'].includes(child.tagName))scene.append(child);
    const nav=element('nav');nav.className='tabs';panel.append(nav,scene);
    for(const [id,title] of [['scene','场景'],['parameters','参数'],['lab','噪波实验室'],['mapping','映射'],['presets','预设 / 文件']]){
      const screen=id==='scene'?scene:element('section');screen.dataset.tab=id;if(id!=='scene'){screen.hidden=true;screen.id=id+'-tab';panel.append(screen);}
      const b=button(nav,title,()=>{tab=id;for(const node of panel.querySelectorAll('[data-tab]'))node.hidden=node.dataset.tab!==id;for(const node of nav.children)node.classList.toggle('active',node===b);api().edit({lab:id==='lab'});});b.classList.toggle('active',id==='scene');
    }
    const extra=element('div');scene.insertBefore(extra,$('demo'));
    row(extra,'群落',select([['auto','自动混合'],...meta.biomes.map((b,i)=>[b,['草甸','森林','高山','海岸','白桦林','针叶林','沙漠','恶地','热带草原','沼泽','丛林'][i]])],api().config.biome,v=>change({biome:v})));
    row(extra,'村庄 / 农田',select([['off','关闭'],['auto','自动'],['showcase','展示']],api().config.villages,v=>change({villages:v})));
    row(extra,'废弃矿井',select([['off','关闭'],['auto','自动'],['showcase','展示']],api().config.mines,v=>change({mines:v})));
    row(extra,'地下视图',select([['surface','完整地形'],['cutaway','剖开一角'],['underground','仅地下']],api().config.view,v=>change({view:v})));
    row(extra,'完整方块精度',checkbox(api().config.full_detail,v=>change({full_detail:v})));
    [...extra.querySelectorAll('select,input')].forEach((control,i)=>control.dataset.sceneProperty=['biome','villages','mines','view','full_detail'][i]);
    const params=element('div');params.id='parameters';$('parameters-tab').append(params);renderParameters();
    const lab=$('lab-tab');row(lab,'切片高度',number(8,-64,256,1,v=>change({slice:v})));row(lab,'叠加音乐映射',checkbox(true,v=>change({overlay:v})));
    for(const title of ['高度场 XZ','密度切片 XZ','密度剖面 XY']){lab.append(element('p',title));const canvas=element('canvas');canvas.width=canvas.height=64;canvas.className='noise';lab.append(canvas);}
    const mapping=element('div');mapping.id='mappings';$('mapping-tab').append(mapping);renderMappings();
    button($('mapping-tab'),'添加映射',()=>{const graph=clone(api().config.mappings);if(graph.rules.length>=128)return;graph.rules.push(clone(meta.presets[0].mappings.rules[0]));change({mappings:graph});renderMappings();});
    const presets=$('presets-tab');row(presets,'原生预设',select(meta.presets.map((p,i)=>[i,['低音山脉','波浪地形','噪波风暴','浮空群岛','水晶脉冲','氛围地貌','极限地形'][i]]),0,v=>{try{load(meta.presets[Number(v)]);}catch(e){report(e);}}));
    const name=element('input');name.id='preset-name';name.value=preset.name;row(presets,'预设名称',name);
    const actions=element('div');actions.className='actions';presets.append(actions);
    const text=element('textarea');text.id='preset-json';text.rows=8;text.placeholder='原生版与网页版共用的预设 JSON';
    button(actions,'导出 JSON',()=>{text.value=JSON.stringify(presetObject(),null,2);const link=element('a');link.href=URL.createObjectURL(new Blob([text.value],{type:'application/json'}));link.download='MusicTerrain-preset.json';link.click();setTimeout(()=>URL.revokeObjectURL(link.href),5000);report('JSON 已显示，可复制或保存');});
    button(actions,'载入 JSON',()=>{try{if(text.value.length>1048576)throw new Error('预设上限为 1 MiB');load(JSON.parse(text.value));}catch(e){report(e);}});
    button(actions,'保存到本机',()=>{try{localStorage.setItem('musicterrain-web-preset-v1',JSON.stringify(presetObject()));report('已保存到当前壁纸的浏览器存储');}catch(e){report(e);}});
    button(actions,'读取本机预设',()=>{try{load(JSON.parse(localStorage.getItem('musicterrain-web-preset-v1')));}catch(e){report(e);}});
    const file=element('input');file.type='file';file.accept='.json';file.onchange=async()=>{try{if(file.files[0].size>1048576)throw new Error('预设上限为 1 MiB');text.value=await file.files[0].text();load(JSON.parse(text.value));}catch(e){report(e);}};row(presets,'导入原生预设',file);presets.append(text);
    const music=element('input');music.type='file';music.accept='.mp3,.wav,.flac,.ogg';music.onchange=()=>fileSelected(music.files[0]).catch(report);row(presets,'本地音乐文件',music);
    const player=element('audio');player.id='music-player';player.controls=true;player.onplay=()=>context?.resume();presets.append(player);
    button(presets,'切回系统音乐',()=>{player.pause();api().setFileAudio(null);report('已切回 Wallpaper Engine 系统音频');});
    const textures=element('input');textures.type='file';textures.multiple=true;textures.webkitdirectory=true;textures.onchange=()=>loadTexture(textures.files).catch(report);row(presets,'本地纹理文件夹',textures);button(presets,'恢复原创材质',()=>{api().setAtlas(originalAtlas);report('已恢复 65 种原创材质');});
    const history=element('div');history.className='actions';panel.append(history);
    button(history,'撤销',()=>{if(!undo.length)return;redo.push(current());api().edit(undo.pop());renderParameters();renderMappings();});
    button(history,'重做',()=>{if(!redo.length)return;undo.push(current());api().edit(redo.pop());renderParameters();renderMappings();});
    const message=element('p');message.id='editor-message';message.setAttribute('role','status');panel.append(message);
    const stats=element('footer');stats.id='scene-stats';panel.append(stats);
  }
  function mesh(message){if(!meta)return;const s=message.stats;$('scene-stats').textContent=`${meta.layers} 种材质 · 树木 ${s.trees} · 植物 ${s.plants} · 房屋 ${s.houses} · 农田 ${s.farms} · 矿井 ${s.mines} · 铁轨 ${s.rails}`;
    if(tab==='lab'&&message.lab.length===64*64*4*3)document.querySelectorAll('canvas.noise').forEach((canvas,i)=>canvas.getContext('2d').putImageData(new ImageData(message.lab.slice(i*16384,(i+1)*16384),64,64),0,0));
  }
  window.MTEditor={ready,mesh,loadPreset:load,report,record,sync};
})();
