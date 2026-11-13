import { Ajv, _ } from 'ajv';
import standaloneCode from 'ajv/dist/standalone/index.js';
import { build } from 'esbuild';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { numericFormats } from './numeric-formats.mjs';

export const validationOptions = {
  strict: true, allErrors: false, coerceTypes: false,
  useDefaults: false, removeAdditional: false,
};
export const validatorTypes = {
  command: 'CommandEnvelope', stream: 'StreamPage', submission: 'SubmissionResult',
  execution: 'CommandExecution', manifest: 'PublicManifest',
  resume: 'SeatResume', bootstrap: 'SeatBootstrap',
};

// Ajv 8.20.0's documented custom-format/standalone API:
// https://github.com/ajv-validator/ajv/blob/v8.20.0/docs/standalone.md
// Bundle its static helper requires too: the published file is real ESM and
// needs neither Node globals nor Ajv's runtime code compiler under browser CSP.
export async function standalone(schemas) {
  const ajv = new Ajv({ ...validationOptions, formats: numericFormats,
    code: { source: true, esm: true, formats: _`require("./numeric-formats.mjs").numericFormats` },
  });
  const names = {};
  for (const name of Object.keys(validatorTypes)) {
    ajv.addSchema(schemas[name], name);
    names[`${name}Validator`] = name;
  }
  ajv.addSchema(schemas.command.properties.command_id, 'commandId');
  ajv.addSchema({ $ref: '#/definitions/DecimalU64Counter', definitions: schemas.stream.definitions }, 'counter');
  names.commandIdValidator = 'commandId'; names.counterValidator = 'counter';
  const result = await build({
    absWorkingDir: path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'),
    stdin: { contents: standaloneCode(ajv, names), sourcefile: 'validators.js',
      resolveDir: path.dirname(fileURLToPath(import.meta.url)) },
    bundle: true, platform: 'browser', format: 'esm', target: 'es2023',
    write: false, metafile: true, legalComments: 'inline',
  });
  const declaration = [
    ...Object.entries(validatorTypes).map(([name, type]) => `import type { ${type} } from './${name}.js';`),
    ...Object.entries(validatorTypes).map(([name, type]) => `export declare function ${name}Validator(value: unknown): value is ${type};`),
    'export declare function commandIdValidator(value: unknown): value is string;',
    'export declare function counterValidator(value: unknown): value is string;',
  ].join('\n') + '\n';
  return { source: result.outputFiles[0].text, declaration };
}
