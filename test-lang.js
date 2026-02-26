const { ModelOperations } = require('@vscode/vscode-languagedetection');

async function test() {
    const ops = new ModelOperations({ minContentSize: 5 });
    
    const tests = [
        '{"test": 1}',
        '{\n  "test": 1\n}',
        '{"test": 1, "foo": "bar"}',
        '// this is a test\nconst x = 1;',
        'console.log("hello");'
    ];
    
    for (const t of tests) {
        const res = await ops.runModel(t);
        console.log(`\nInput: ${JSON.stringify(t)}`);
        console.log(`Results:`, res.slice(0, 3));
    }
}
test().catch(console.error);
