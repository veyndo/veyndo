/** Architecture gate rejects real source regressions, including cycles and ownership violations. */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
const directory=fs.mkdtempSync(path.join(os.tmpdir(),'commerce-architecture-'));
const guard=path.resolve(import.meta.dirname,'../scripts/architecture.mjs');
function file(name,text){const target=path.join(directory,name);fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,text);fs.writeFileSync(path.join(path.dirname(target),'README.md'),'# Fixture contract\n');}
function check(){return spawnSync(process.execPath,[guard,directory],{encoding:'utf8'});}
try{
 file('shared/api/base.ts','/** Shared fixture contract. */\nexport const value=1;\n');
 file('admin/orders/view.ts','/** Consumer fixture. */\nimport {value} from "../../shared/api/base";\nexport const read=()=>value;\n');
 assert.equal(check().status,0);
 file('shared/api/base.ts','/** Forbidden app dependency. */\nimport {read} from "../../admin/orders/view";\nexport const value=read();\n');
 assert.match(check().stderr,/shared boundary/);assert.match(check().stderr,/Runtime import cycle/);
 file('shared/api/base.ts','/** Missing import. */\nimport "./missing";\n');assert.match(check().stderr,/cannot resolve/);
 file('shared/api/base.ts','export const missingComment=1;\n');assert.match(check().stderr,/responsibility comment/);
 file('shared/api/base.ts','/** Documented fixture. */\nexport const value=1;\n');
 const sdk=path.resolve(import.meta.dirname,'../../extensions/sdk');
 file('shared/api/base.ts',`/** Shared immutable native asset contract. */\nimport contract from ${JSON.stringify(path.relative(path.join(directory,'shared/api'),path.join(sdk,'native-asset-actions.json')))};\nexport const actions=contract;\n`);
 assert.equal(check().status,0,check().stderr);
 file('shared/api/base.ts',`/** Other SDK implementation remains outside the boundary. */\nimport client from ${JSON.stringify(path.relative(path.join(directory,'shared/api'),path.join(sdk,'browser.js')))};\nexport const actions=client;\n`);
 assert.match(check().stderr,/outside the source ownership tree/);
 file('shared/api/base.ts','/** Documented fixture. */\nexport const value=1;\n');
 file('kit/tokens/index.ts','/** Token fixture. */\nexport const space=4;\n');
 file('kit/ui/index.ts','/** Kit fixture. */\nimport {space} from "../tokens";\nexport const gap=space;\n');
 file('kit/ui/ui.css','/* Kit fixture styles. */\n.vy-a { color: var(--vy-color-text); }\n');
 file('admin/orders/view.ts','/** Consumer fixture. */\nimport {gap} from "../../kit/ui";\nexport const read=()=>gap;\n');
 assert.equal(check().status,0,check().stderr);
 file('kit/ui/ui.css','/* Kit fixture styles. */\n.vy-a { color: #fff; }\n');assert.match(check().stderr,/colour literal/);
 file('kit/ui/ui.css','/* Kit fixture styles. */\n.vy-a { color: rgb(0 0 0); }\n');assert.match(check().stderr,/colour literal/);
 file('kit/ui/ui.css','/* Kit fixture styles. */\n.vy-a { color: var(--vy-color-text); }\n');
 file('kit/ui/Button.ts','/** Kit internal. */\nexport const b=1;\n');
 file('admin/orders/view.ts','/** Consumer fixture. */\nimport {b} from "../../kit/ui/Button";\nexport const read=()=>b;\n');assert.match(check().stderr,/internals; import its index/);
 file('admin/orders/view.ts','/** Consumer fixture. */\nexport const read=()=>1;\n');
 file('kit/tokens/index.ts','/** Token fixture importing upward. */\nimport {gap} from "../ui";\nexport const space=gap;\n');assert.match(check().stderr,/kit layering/);
 file('kit/tokens/index.ts','/** Token fixture importing the app. */\nimport {value} from "../../shared/api/base";\nexport const space=value;\n');assert.match(check().stderr,/imports outside kit/);
 file('kit/tokens/index.ts','/** Token fixture. */\nexport const space=4;\n');
 assert.equal(check().status,0,check().stderr);
 fs.unlinkSync(path.join(directory,'shared/api/README.md'));assert.match(check().stderr,/folder contract/);
 console.log('PASS source ownership, runtime-cycle, missing-import, comment, documentation and kit layering/token negative controls');
}finally{fs.rmSync(directory,{recursive:true,force:true});}
