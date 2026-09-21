#!/usr/bin/env python3
"""Source-bound exploratory nested-image PDF controls, never changes production."""
import sys
if not __debug__ or sys.flags.optimize: raise SystemExit('Optimized Python forbidden')
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import zlib
import types

HERE = Path(__file__).resolve().parent
c = types.ModuleType('jpeg187_collect')
c.__file__ = str(HERE / 'collect.py')
exec(compile((HERE / 'collect.py').read_bytes(), c.__file__, 'exec'), c.__dict__)
ROOT, ENV = c.ROOT, c.ENV
def b64(data): return base64.b64encode(data).decode()
def png(width, height, depth, color, samples):
    def chunk(tag, data): return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data))
    channels = {2: 3, 6: 4}[color]
    stride = width * channels * (depth // 8)
    raw = b''.join(b'\0' + samples[y * stride:(y + 1) * stride] for y in range(height))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, depth, color, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')
def gif(width, height, indices):
    # Four palette entries; clear before every sample keeps code width exactly 3.
    codes = [v for index in indices for v in (4, index)] + [5]
    bits = sum(code << (3 * i) for i, code in enumerate(codes))
    data = bits.to_bytes((len(codes) * 3 + 7) // 8, 'little')
    blocks = b''.join(bytes([len(data[i:i+255])]) + data[i:i+255] for i in range(0, len(data), 255))
    return b'GIF89a' + struct.pack('<HHBBB', width, height, 0x81, 0, 0) + bytes([255,0,0,0,255,0,0,0,255,255,255,0]) + b'\x2c' + struct.pack('<HHHHB', 0,0,width,height,0) + b'\x02' + blocks + b'\0\x3b'
def scene(svg, width, height):
    return {'type': 'excalidraw', 'version': 2, 'elements': [{'id':'image','type':'image','x':0,'y':0,'width':width,'height':height,'angle':0,'opacity':100,'fileId':'asset','status':'saved','scale':[1,1],'seed':1,'updated':1}], 'appState': {}, 'files': {'asset': {'id':'asset','mimeType':'image/svg+xml','dataURL':'data:image/svg+xml;base64,'+b64(svg),'created':1}}}

def author():
    rows = []
    inputs = c.read(ROOT / 'tests/oracle/jpeg-color/inputs.json')['cases']
    reference = {r['id']: r for r in c.read(ROOT / 'tests/oracle/jpeg-color/reference.json')['cases']}
    def add(name, media, data, width, height, samples=None, blur=False):
        control = HERE / 'nested/inputs' / name
        definitions = '<defs><filter id="fx" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="0.5"/></filter></defs>' if blur else ''
        effect = ' filter="url(#fx)"' if blur else ''
        svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">{definitions}<g{effect}><image x="0" y="0" width="{width}" height="{height}" href="data:{media};base64,{b64(data)}"/></g></svg>').encode()
        c.put(control / 'source.bin', data)
        c.put(control / 'nested.svg', svg)
        c.json_new(control / 'scene.json', scene(svg, width, height))
        if samples is not None: c.put(control / 'independent-rgba.bin', samples)
        rows.append({'id':name,'media_type':media,'width':width,'height':height,'filter':blur,'source_sha256':c.sha(data),'scene':str((control / 'scene.json').relative_to(ROOT)), 'expected_report': 'one nested-SVG filter fallback diagnostic' if blur else 'empty diagnostics; no placeholder/recovery', 'independent_samples':samples is not None})
    for identity in ['rgb-1x1-baseline-edges','cmyk-1x1-baseline-ramp','ycck-1x1-progressive-ramp','ycbcr-4x2-baseline-edges','ycck-2x4-baseline-edges']:
        row = next(r for r in inputs if r['id'] == identity)
        add('jpeg-' + identity, 'image/jpeg', base64.b64decode(row['jpeg_base64']), row['width'], row['height'])
        c.json_new(HERE / 'nested/inputs' / ('jpeg-' + identity) / 'frozen-browser-reference.json', reference[identity]['reference'])
    w, h = 16, 12
    palette = [(230,25,60),(20,210,80),(40,65,230),(220,195,30)]
    rgb = bytes(v for y in range(h) for x in range(w) for v in palette[(x//4+y//3)%4])
    opaque = bytes(v for p in range(w*h) for v in (*rgb[p*3:p*3+3],255))
    rgba = bytes(v for p in range(w*h) for v in (*rgb[p*3:p*3+3], [0,64,173,255][(p%w)//4]))
    add('png-rgb8', 'image/png', png(w,h,8,2,rgb),w,h,opaque)
    add('png-rgba8', 'image/png', png(w,h,8,6,rgba),w,h,rgba)
    add('png-rgb16', 'image/png', png(w,h,16,2,b''.join(bytes([v,v]) for v in rgb)),w,h,opaque)
    indices = [(x//4+y//3)%4 for y in range(h) for x in range(w)]
    gp = [(255,0,0),(0,255,0),(0,0,255),(255,255,0)]
    add('gif-palette', 'image/gif', gif(w,h,indices),w,h,bytes(v for i in indices for v in (*gp[i],255)))
    add('png-alpha-filter', 'image/png', png(w,h,8,6,rgba),w,h,None,True)
    c.json_new(HERE / 'nested/controls.json', rows)
    print(f'Authored {len(rows)} controls independently of renderer outputs.')

def model(pdf, output):
    qpdf = ROOT / 'target/pdf-harness/qpdf/bin/qpdf'
    result = c.command([qpdf, '--json=2', '--json-stream-data=inline', '--decode-level=all', pdf])
    c.put(output, result.stdout)
    record = json.loads(result.stdout)
    objects = record['qpdf'][1]
    pages = [key for key, obj in objects.items() if obj.get('value',{}).get('/Type') == '/Page']
    c.require(len(pages) == 1, 'expected one page')
    page = objects[pages[0]]['value']
    visited, stack = set(), [pages[0]]
    def references(value):
        if isinstance(value, str) and re.fullmatch(r'\d+ \d+ R',value): yield 'obj:' + value
        elif isinstance(value, dict):
            for k,v in value.items():
                if k != '/Parent': yield from references(v)
        elif isinstance(value,list):
            for v in value: yield from references(v)
    while stack:
        key = stack.pop()
        if key in visited: continue
        visited.add(key)
        stack.extend(references(objects[key]))
    images, forms = [], []
    for key in sorted(visited):
        stream = objects[key].get('stream',{})
        dictionary = stream.get('dict',{})
        if dictionary.get('/Subtype') == '/Form': forms.append(key)
        if dictionary.get('/Subtype') == '/Image':
            data = base64.b64decode(stream['data'])
            images.append({'object':key,'dictionary':dictionary,'decoded_stream_length':len(data),'decoded_stream_sha256':c.sha(data)})
    c.require(forms and images, 'nested Form/image branch missing')
    c.require(page.get('/Rotate',0)==0 and page.get('/UserUnit',1)==1, 'page geometry changes')
    catalogs = [o['value'] for o in objects.values() if o.get('value',{}).get('/Type') == '/Catalog']
    c.require(len(catalogs)==1 and catalogs[0].get('/OutputIntents'), 'catalog OutputIntent missing')
    return {'page':page,'reachable_forms':forms,'reachable_images':images,'catalog_output_intents':catalogs[0]['/OutputIntents']}

def collect(label, stage=None, freeze_file=None):
    final = freeze_file is not None
    stage = stage or (c.OLD if label == 'old' else c.CURRENT)
    before = c.tracked_state(stage)
    if final:
        freeze=c.read(freeze_file)
        c.require(freeze.get('final_current_capture_authorized') is True, 'parent final freeze required')
        recorded=freeze['producer_files']
        required={d['path'] for d in c.read(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v8/native-capture.json')['producer_sources']}
        c.require(required<=set(recorded),'incomplete native freeze')
        for path,expected in recorded.items(): c.require(c.sha((ROOT/path).read_bytes())==expected,'live source drift '+path)
    else:
        recorded = c.read(HERE / ('old' if label=='old' else 'exploratory-current') / 'producer-files.json')
    # Package consumer lock is the only permitted isolated-stage metadata change.
    for path, expected in recorded.items():
        if path != 'tests/package-consumer/Cargo.lock': c.require(before[path]==expected, 'staged source drift '+path)
    c.command(['cargo','build','--release','--locked','-p','excalidraw-render-cli','--message-format=json-render-diagnostics'],stage,'nested-'+label+'-build')
    executable = stage / 'target/release/excalidraw-render'
    rows = []
    for control in c.read(HERE / 'nested/controls.json'):
        name = control['id']; directory = HERE / 'nested' / label / name
        directory.mkdir(parents=True, exist_ok=False)
        scene_bytes = (ROOT / control['scene']).read_bytes()
        row = dict(control, format_runs=[])
        for fmt in ['svg','png','pdf']:
            first=None
            for repeat in range(2):
                result = subprocess.run([str(executable),'-','--output','-','--format',fmt,'--report-format','json','--padding','0','--no-background'],input=scene_bytes,env=ENV,capture_output=True)
                c.put(directory / f'{fmt}-{repeat}.artifact',result.stdout)
                c.put(directory / f'{fmt}-{repeat}.report.json',result.stderr)
                current = (result.returncode,result.stdout,result.stderr)
                if first is not None: c.require(first==current, 'nondeterminism '+name)
                first=current
            report=json.loads(first[2])
            row['format_runs'].append({'format':fmt,'exit_status':first[0],'artifact_sha256':c.sha(first[1]),'report_sha256':c.sha(first[2]),'report':report,'repeat_equal':True})
        if row['format_runs'][-1]['exit_status'] == 0:
            try:
                row['pdf_semantics']=model(directory / 'pdf-0.artifact',directory / 'qpdf.json')
                page=row['pdf_semantics']['page']
                c.require(page['/MediaBox']==[0,0,control['width']*.75,control['height']*.75], 'wrong media box')
                for viewer in ['pdfium','pdfjs']:
                    pixels=[]
                    for repeat in range(2):
                        output=directory / f'{viewer}-{repeat}.rgba'
                        result=c.command(['bash',f'tools/pdf-harness/run-{viewer}.sh','render',directory/'pdf-0.artifact',output,str(control['width']),str(control['height'])],allow_failure=True)
                        c.put(directory/f'{viewer}-{repeat}.stdout',result.stdout)
                        c.put(directory/f'{viewer}-{repeat}.stderr',result.stderr)
                        c.require(result.returncode==0,viewer+' failed: '+result.stderr.decode())
                        pixels.append(output.read_bytes())
                    c.require(pixels[0]==pixels[1],viewer+' nondeterminism')
                    row[viewer]={'rgba_sha256':c.sha(pixels[0]),'repeat_equal':True}
            except (ValueError,KeyError) as error: row['diagnostic_failure']=str(error)
        rows.append(row)
        print(label,name,'captured',row.get('diagnostic_failure',''),flush=True)
    c.require(c.tracked_state(stage)==before,'stage mutated while collecting')
    if final:
        for path,expected in recorded.items(): c.require(c.sha((ROOT/path).read_bytes())==expected,'live source changed during collection '+path)
    c.json_new(HERE/'nested'/label/'collection.json',{'status':'final-freeze-unapproved' if final else ('historical-head' if label=='old' else 'exploratory-current-not-final'),'approval':None,'producer_files':before,'executable_sha256':c.sha(executable.read_bytes()),'collector':c.desc(ROOT,str(Path(__file__).relative_to(ROOT))),'controls':rows})

def compare(label='exploratory'):
    old=c.read(HERE/'nested/old/collection.json'); new=c.read(HERE/'nested'/label/'collection.json')
    changes=[]
    for a,b in zip(old['controls'],new['controls']):
        c.require(a['id']==b['id'],'control mismatch')
        row={'id':a['id'],'diagnostics_before':[r['report'].get('diagnostics') for r in a['format_runs']],'diagnostics_after':[r['report'].get('diagnostics') for r in b['format_runs']],'formats':[]}
        for x,y in zip(a['format_runs'],b['format_runs']): row['formats'].append({'format':x['format'],'artifact_equal':x['artifact_sha256']==y['artifact_sha256'],'report_equal':x['report_sha256']==y['report_sha256']})
        for viewer in ['pdfium','pdfjs']:
            if viewer in a and viewer in b:
                x=(HERE/'nested/old'/a['id']/f'{viewer}-0.rgba').read_bytes(); y=(HERE/'nested'/label/a['id']/f'{viewer}-0.rgba').read_bytes()
                row[viewer]={'exact_changed_pixels':sum(x[i:i+4]!=y[i:i+4] for i in range(0,len(x),4)),'bytes_equal':x==y}
        row['failures']=[r.get('diagnostic_failure') for r in [a,b] if r.get('diagnostic_failure')]
        changes.append(row)
    c.json_new(HERE/'nested'/f'comparison.{label}.json',{'approval':None,'final_freeze_recollection_required':label=='exploratory','controls':changes})
    print(json.dumps(changes,indent=2))

def qualify(label='exploratory'):
    executable=ROOT/'target/jpeg187-diagnostic-compare/release/jpeg187-diagnostic-compare'
    rows=[]
    for control in c.read(HERE/'nested/controls.json'):
        name=control['id']; source=HERE/'nested/inputs'/name
        width,height=control['width'],control['height']
        expected=None
        if control['independent_samples']:
            expected=(source/'independent-rgba.bin').read_bytes()
            expected=bytes(v for i in range(0,len(expected),4) for v in (expected[i:i+4] if expected[i+3] else b'\0\0\0\0'))
        elif name.startswith('jpeg-'):
            reference=c.read(source/'frozen-browser-reference.json')
            browser=source/'browser-svg.png'
            if browser.exists(): c.require(browser.read_bytes()==base64.b64decode(reference['svg_png']),'stale browser source')
            else: c.put(browser,base64.b64decode(reference['svg_png']))
            if not (source/'browser-svg.rgba').exists():
                result=c.command([executable,'decode',browser,source/'browser-svg.rgba'])
                size=json.loads(result.stdout); c.require(size=={'width':width,'height':height},'browser extent')
            expected=(source/'browser-svg.rgba').read_bytes()
        if expected is not None:
            if (source/'expected-canonical.rgba').exists(): c.require((source/'expected-canonical.rgba').read_bytes()==expected,'stale expected samples')
            else: c.put(source/'expected-canonical.rgba',expected)
        for producer in (['old','exploratory'] if label=='exploratory' else [label]):
            directory=HERE/'nested'/producer/name
            model=c.read(directory/'qpdf.json')['qpdf'][1]
            def obj(ref): return model['obj:'+ref]
            page=next(o['value'] for o in model.values() if o.get('value',{}).get('/Type')=='/Page')
            catalog=next(o['value'] for o in model.values() if o.get('value',{}).get('/Type')=='/Catalog')
            profile=obj(obj(catalog['/OutputIntents'][0])['value']['/DestOutputProfile'])['stream']
            c.require(profile['dict']['/N']==3 and profile['dict']['/Alternate']=='/DeviceRGB','wrong output ICC')
            visited=set(); called=[]
            def traverse(resources, data):
                for key in re.findall(rb'/([^\s/]+)\s+Do\b',data):
                    key='/'+key.decode(); c.require(key in resources.get('/XObject',{}),'unbound Do')
                    reference=resources['/XObject'][key]; called.append(reference)
                    if reference in visited: continue
                    visited.add(reference); stream=obj(reference)['stream']; d=stream['dict']
                    if d['/Subtype']=='/Form': traverse(d.get('/Resources',{}),base64.b64decode(stream['data']))
            traverse(page['/Resources'],base64.b64decode(obj(page['/Contents'])['stream']['data']))
            images=[ref for ref in visited if obj(ref)['stream']['dict'].get('/Subtype')=='/Image']
            c.require(len(images)==1,'expected one bound color image')
            image=obj(images[0])['stream']; d=image['dict']; raw=base64.b64decode(image['data'])
            c.require(d['/Width']==width and d['/Height']==height and d['/BitsPerComponent'] in [8,16], 'image geometry/depth')
            c.require(d['/ColorSpace']=='/DeviceRGB','unexpected nested image colorspace')
            step=d['/BitsPerComponent']//8
            c.require(len(raw)==width*height*3*step,'color sample extent')
            if step==2: raw=bytes(raw[i] for i in range(0,len(raw),2))
            alpha=bytes([255])*(width*height)
            if '/SMask' in d:
                mask=obj(d['/SMask'])['stream']; md=mask['dict']
                c.require(md['/Width']==width and md['/Height']==height and md['/ColorSpace']=='/DeviceGray','mask binding')
                alpha=base64.b64decode(mask['data'])
                if md['/BitsPerComponent']==16: alpha=bytes(alpha[i] for i in range(0,len(alpha),2))
                c.require(len(alpha)==width*height,'alpha sample extent')
            rgba=bytes(v for i,a in enumerate(alpha) for v in ((*raw[i*3:i*3+3],a) if a else (0,0,0,0)))
            c.put(directory/'bound-pdf-samples.rgba',rgba)
            result={'id':name,'label':producer,'page_form_Do_image_binding':True,'output_intent_bound':True,'soft_mask':d.get('/SMask'),'image_bits':d['/BitsPerComponent'],'image_samples_sha256':c.sha(rgba),'comparisons':[]}
            for fmt in ['svg','png','pdf']:
                report=c.read(directory/f'{fmt}-0.report.json')
                c.require(report['status']=='success','render failure')
                diagnostics=report['diagnostics']
                if control['filter'] and fmt=='pdf': c.require(len(diagnostics)==1 and diagnostics[0]['code']=='pdf.raster-fallback','wrong fallback report')
                else: c.require(diagnostics==[],'unexpected recovery or warnings')
            if expected is not None:
                for target in ['bound-pdf-samples.rgba','pdfium-0.rgba','pdfjs-0.rgba']:
                    mode='pdfjs' if target.startswith('pdfjs') else 'canonical'
                    metric=json.loads(c.command([executable,width,height,source/'expected-canonical.rgba',directory/target,mode]).stdout)
                    result['comparisons'].append({'target':target,**metric})
            rows.append(result)
    c.json_new(HERE/'nested'/f'qualification.{label}.json',{'approval':None,'final_freeze_recollection_required':label=='exploratory','records':rows})
    for r in rows:
        print(r['label'],r['id'],[(v['target'],v['canonical_pass'],v['changed_pixels'],v['ssim_bits']) for v in r['comparisons']])

if __name__=='__main__':
    if sys.argv[1]=='author': author()
    elif sys.argv[1] in ['old','exploratory']: collect(sys.argv[1])
    elif sys.argv[1]=='compare': compare(sys.argv[2] if len(sys.argv)>2 else 'exploratory')
    elif sys.argv[1]=='qualify': qualify(sys.argv[2] if len(sys.argv)>2 else 'exploratory')
    elif sys.argv[1]=='final': collect(sys.argv[3],Path(sys.argv[2]),Path(sys.argv[4]))
