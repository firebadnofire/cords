// Generate distinctive, indefinitely looping disposable desktop-test images.
// No production input or user photo is included.
import {readFile, writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
import {AvatarJxlEncoder} from '../../bins/cords-client/ui/src/avatarJxlEncoder.ts';
const root=resolve(process.argv[2] ?? 'assets/avatar-codec');
const {default:create}=await import(pathToFileURL(`${root}/cords-jxl.mjs`).href);
for(const [name,offset] of [['a',0],['b',90],['static',0],['finite',0]] as const){
  const module=await create({wasmBinary:await readFile(`${root}/cords-jxl.wasm`),wasmMemory:new WebAssembly.Memory({initial:512,maximum:4096})});
  const encoder=new AvatarJxlEncoder(module,{width:64,height:64,bits:8,animated:name!=='static',loops:name==='finite'?1:0,ticksPerSecondNumerator:10,ticksPerSecondDenominator:1},new Uint8Array(5000000));
  try{
    for(let frame=0;frame<(name==='static'?1:4);frame++){
      const pixels=new Uint8Array(64*64*4);
      for(let y=0;y<64;y++)for(let x=0;x<64;x++){
        const at=(y*64+x)*4;
        pixels[at]=40+offset; pixels[at+1]=80+frame*40; pixels[at+2]=210-offset;
        if(x>=frame*12+4 && x<frame*12+12 && y>=12 && y<52) pixels.fill(245,at,at+3);
        pixels[at+3]=255;
      }
      encoder.push(pixels,name==='static'?0:5,name==='static'||frame===3);
    }
    await writeFile(`crates/cords-avatar-codec/tests/fixtures/${name==='static'||name==='finite'?name:`looping-${name}`}.jxl`,encoder.finish());
  }finally{encoder.dispose();}
}
