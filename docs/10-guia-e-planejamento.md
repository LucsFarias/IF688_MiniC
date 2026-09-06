# Guia de estudo e evolução do compilador Mini-C

Este texto é um roteiro de leitura do repositório e uma proposta técnica para
a primeira entrega do projeto da disciplina de Compiladores. O foco está na
árvore sintática abstrata (AST), no parser e na extensão da linguagem com
tipos algébricos e pattern matching.

## 1. O problema que estamos estudando

O projeto implementa uma linguagem pequena, parecida com C, em Rust. Ela já
possui:

- literais `int`, `float`, `bool` e `str`;
- arrays, indexação e atribuição;
- declarações de funções em estilo C;
- `if`, `while`, blocos e `return`;
- operações aritméticas, relacionais e booleanas;
- verificação de tipos;
- interpretador tree-walking;
- uma representação inicial de código de três endereços (TAC).

O trabalho discutido com o professor acrescenta duas construções relacionadas:

1. **tipo algébrico**, semelhante a `enum` de Rust;
2. **pattern matching**, semelhante à expressão `match` de Rust.

Para a primeira entrega, o escopo é alterar a AST e o parser. Isso não elimina
a necessidade de pensar nas fases seguintes: uma AST que não consiga
representar claramente os novos conceitos produzirá retrabalho no type
checker, no interpretador e no gerador de código.

## 2. Visão geral do pipeline

O caminho atual é:

```text
arquivo .minic
    |
    v
src/main.rs: leitura do arquivo e seleção de --check/--run
    |
    v
src/parser/: texto -> UncheckedProgram (AST com Ty = ())
    |
    v
src/semantic/: UncheckedProgram -> CheckedProgram (Ty = Type)
    |
    +--> --check: termina aqui
    |
    v
src/interpreter/: CheckedProgram -> efeitos e valores
    |
    v
saída do programa ou RuntimeError
```

O gerador de TAC está em `src/codegen/` e `src/ir/tac.rs`, mas ainda é
incompleto e não está conectado ao fluxo principal do CLI.

Essa separação é uma decisão pedagógica importante: cada fase recebe uma
representação e possui uma responsabilidade. O parser reconhece forma; o
type checker verifica significado; o interpretador executa.

## 3. Onde começar a leitura

### 3.1 AST: `src/ir/ast.rs`

Este é o contrato entre as fases.

`Type` representa os tipos atualmente conhecidos:

```rust
Type::Unit
Type::Int
Type::Float
Type::Bool
Type::Str
Type::Array(Box<Type>)
Type::Fun(Vec<Type>, Box<Type>)
Type::Any
```

`Expr<Ty>` representa expressões. Entre suas variantes estão `Literal`,
`Ident`, operadores, `Call`, `ArrayLit` e `Index`.

`ExprD<Ty>` adiciona a decoração:

```rust
pub struct ExprD<Ty> {
    pub exp: Expr<Ty>,
    pub ty: Ty,
}
```

No parser, `Ty` é `()`: temos `UncheckedExpr`. Após a análise semântica,
`Ty` é `Type`: temos `CheckedExpr`. O mesmo mecanismo vale para statements e
programas.

Essa parametrização é uma das melhores ideias do projeto. Ela evita manter
duas árvores duplicadas e faz o Rust impedir que o interpretador receba um
programa não verificado.

`Statement<Ty>` contém declarações, atribuições, blocos, chamadas, `if`,
`while` e `return`. `FunDecl<Ty>` contém nome, parâmetros, retorno e corpo.
`Program<Ty>` é uma lista de funções; a execução procura `main`.

### 3.2 Parser: `src/parser/`

O parser usa `nom` e está dividido por categoria:

```text
program.rs       programa completo
  functions.rs   funções e nomes de tipos
    statements.rs statements e lvalues
      expressions.rs expressões e precedência
        literals.rs literais
        identifiers.rs identificadores
```

Os pontos de entrada públicos são reexportados em `src/parser/mod.rs`:
`program`, `fun_decl`, `statement`, `assignment`, `expression`, `literal` e
`identifier`.

Um parser `nom` recebe `&str` e retorna `IResult<&str, T>`. O primeiro item é
o restante da entrada; o segundo é o valor construído. Portanto, consumir a
entrada inteira é uma responsabilidade importante da chamada de alto nível.

### 3.3 Semântica e execução

`src/semantic/type_checker.rs` registra funções e variáveis em
`Environment<Type>`. Ele percorre a AST, verifica compatibilidade e constrói
uma AST decorada com tipos.

`src/interpreter/eval_expr.rs` avalia expressões e
`src/interpreter/exec_stmt.rs` executa statements. O runtime usa `Value` em
`src/interpreter/value.rs`, que é outro enum Rust com `Int`, `Float`, `Bool`,
`Str`, `Array`, `Void` e funções.

Observe a distinção didática:

- `Type` diz o que um valor pode ser;
- `Value` é o valor concreto durante a execução;
- `Expr`/`Statement` dizem como o programa foi estruturado.

## 4. Como o parser atual funciona

### 4.1 Funções e programa

Em `src/parser/functions.rs`, `type_name` reconhece os tipos primitivos,
`void` e arrays. `fun_decl` reconhece a forma:

```text
ReturnType name(Type param, ...) statement
```

Em `src/parser/program.rs`, o programa é construído com `many0(fun_decl)`.
Isso permite zero ou mais funções, mas também faz o parser parar quando uma
declaração falha. Por isso, a função `program` deve ser chamada com uma
verificação de entrada restante, preferencialmente usando `all_consuming` ou
uma regra equivalente.

Este é um problema real: `Ok((_, prog))` em `src/main.rs` descarta o restante
da entrada. Um arquivo parcialmente reconhecido pode parecer válido. A
primeira melhoria de robustez deve ser rejeitar qualquer texto não consumido.

### 4.2 Statements

Em `src/parser/statements.rs`, `statement` tenta alternativas para bloco,
`if`, `while`, `return`, declaração, chamada e atribuição.

A ordem importa. Declaração deve ser tentada antes de atribuição. O lvalue
aceita `x` e cadeias como `a[i][j]`; o parser constrói `Index(Index(a, i), j)`.

A linguagem atual exige blocos em `if` e `while`:

```c
if condition { ... }
while condition { ... }
```

### 4.3 Expressões e precedência

Em `src/parser/expressions.rs`, a precedência está codificada na cadeia:

```text
expression
  logical_or
    logical_and
      logical_not
        relational
          additive
            multiplicative
              unary
                primary
                  atom
```

Assim, `1 + 2 * 3` vira `Add(1, Mul(2, 3))`. Os loops com acumulador
implementam associatividade à esquerda: `1 - 2 - 3` vira
`Sub(Sub(1, 2), 3)`.

Uma nova construção que seja expressão deve entrar em um nível compatível da
cadeia. `match` normalmente ocupa o lugar de uma expressão atômica ou
primária, pois `match e { ... }` produz um valor e pode aparecer dentro de
uma declaração, chamada ou retorno.

## 5. Proposta de sintaxe para a primeira entrega

Recomendo uma sintaxe próxima de Rust, mas reduzida para permanecer coerente
com Mini-C. A proposta é:

```c
type OptionInt {
  None;
  Some(int value);
}

OptionInt choose(bool flag) {
  if flag { return Some(42); }
  return None;
}

void main() {
  OptionInt result = choose(true);
  int answer = match result {
    Some(value) => value;
    None => 0;
  };
  print(answer);
}
```

Decisões da proposta:

- `type Nome { ... }` declara um tipo algébrico;
- cada linha declara um construtor;
- `None` é construtor sem campos;
- `Some(int value)` é construtor com campos nomeados e tipados;
- construir um valor usa o mesmo nome do construtor, como uma chamada;
- `match` é uma expressão;
- cada braço possui `padrao => expressao`;
- `;` separa construtores e braços, mantendo o estilo atual do Mini-C.

Para a primeira implementação, os padrões podem ser limitados a:

```text
_                         curinga
ident                     variável que captura um valor
Construtor                construtor sem payload
Construtor(padrao, ...)   construtor com payload
literal                   opcional, para futura extensão
```

Não recomendo começar com padrões arbitrariamente recursivos, guards (`if` no
braço), referências ou desestruturação de arrays. Cada item aumenta a
gramática e o trabalho do type checker sem ser necessário para demonstrar a
ideia central.

### 5.1 Parecer sobre a proposta alternativa da equipe

A proposta apresentada pela equipe tem bom valor pedagógico e é tecnicamente
viável. Ela escolhe uma notação familiar de linguagens funcionais e Rust,
expõe bem a ideia de soma de tipos e mostra os três casos essenciais:

- variantes sem dados, como `Red` e `None`;
- variantes com payload, como `Circle(int radius)`;
- mistura de construtores com e sem payload, como `Option`.

Também é positiva a escolha de payload posicional. Para a primeira entrega,
`Circle(int radius)` e `Rectangle(int width, int height)` são mais simples de
parsear e de representar do que campos nomeados com uma sintaxe adicional.
O padrão `_` é uma boa primeira extensão porque demonstra cobertura sem exigir
um algoritmo completo de análise de exaustividade.

Como professor, eu aprovaria essa direção como proposta inicial, mas pediria
que a equipe corrigisse as inconsistências abaixo antes de implementá-la.

#### Inconsistências que precisam ser resolvidas

1. **Declaração de tipo com `=` versus sem `=`.** O exemplo usa
  `type Color = Red | Green | Blue;`, enquanto a proposta anterior usava
  `type Nome { ... }`. Escolham uma forma única. Recomendo manter o `=` e o
  `|`: eles deixam explícito que se trata de uma soma de variantes e tornam o
  formato compacto.
2. **Separadores de braços.** Os exemplos usam vírgulas, mas o Mini-C atual
  usa `;` para terminar statements. Como `match` será uma expressão, vírgulas
  são defensáveis; ainda assim, a gramática deve dizer isso explicitamente e
  aceitar uma única convenção. Recomendo `,` entre braços e `;` depois da
  declaração que contém o `match`.
3. **`match` como statement ou expressão.** `area` usa `match` para produzir
  um inteiro, portanto `match` deve ser uma expressão. Já `print_color` e os
  exemplos de `Option` usam o match apenas por seus efeitos. Isso continua
  válido se uma expressão puder ocupar a posição de statement, mas essa regra
  precisa ser declarada; alternativamente, crie futuramente um statement
  `match` separado. Para a primeira entrega, recomendo apenas `Expr::Match`.
4. **`printf` não existe hoje no projeto.** A stdlib atual oferece `print` e
  outras funções registradas em `src/stdlib/`. Use `print` nos exemplos da
  especificação ou inclua `printf` como uma decisão independente de biblioteca
  padrão. Não misture a extensão de ADTs com uma mudança silenciosa da API.
5. **`main` não respeita o contrato atual.** O exemplo declara `int main()`;
  atualmente o type checker exige `void main()` sem parâmetros. Para testar a
  sintaxe no estado atual, use `void main()`. Se `int main()` for desejado,
  isso é uma mudança separada no contrato de entrada do programa.
6. **`Option` não informa o tipo do payload.** Como exemplo didático,
  `type Option = Some(int val) | None;` é compreensível. Como linguagem
  geral, porém, ele é um tipo concreto de inteiro, não um tipo genérico.
  Recomendo chamá-lo `OptionInt` nesta etapa, ou deixar explícito que
  generics não fazem parte do escopo.
7. **A expressão `3 * r * r` usa uma aproximação de pi.** Isso não é problema
  para o parser, mas `Circle` e `area` misturam a demonstração de ADT com uma
  decisão numérica. Para um teste de parser, prefira `r * r` ou declare que a
  função calcula uma área inteira aproximada.

#### Versão normalizada recomendada

Mantendo a ideia da equipe e ajustando-a ao Mini-C atual, a especificação pode
usar:

```c
type Color = Red | Green | Blue;

type Shape = Circle(int) | Rectangle(int, int);

type OptionInt = Some(int) | None;

void print_color(Color c) {
  match (c) {
   Red => print("vermelho");
   Green => print("verde");
   Blue => print("azul");
  }
}

int area(Shape s) {
  return match (s) {
   Circle(r) => r * r;
   Rectangle(w, h) => w * h;
  };
}

void main() {
  OptionInt a = Some(42);
  int value = match (a) {
   Some(v) => v;
   None => 0;
  };
  print(value);
}
```

Aqui adotamos `;` para separar braços, pois ele combina com a convenção já
presente no parser e evita introduzir duas funções para uma mesma pontuação.
Também mantemos `match` como expressão: um match usado apenas por efeitos é
aceito como expressão de statement numa etapa posterior, ou pode ser colocado
em uma declaração descartável enquanto o parser inicial ainda for restrito.

Se a equipe preferir vírgulas, a decisão também é válida. O essencial é que a
gramática, os testes e os exemplos usem sempre o mesmo separador.

### 5.2 O que pode ser ignorado na primeira entrega

Para não aumentar o escopo artificialmente, os seguintes itens podem ficar
fora da primeira entrega:

- tipos genéricos, como `Option<T>`;
- parâmetros nomeados em variantes;
- variantes recursivas, como uma lista que contém outra lista;
- padrões literais (`Some(0)`), guards e condições nos braços;
- análise sofisticada de padrões inalcançáveis;
- inferência de tipo para construtores sem contexto;
- representação em memória para RISC-V, WebAssembly ou LLVM;
- `printf`, formatação e qualquer alteração da stdlib;
- otimizações de match e compilação direta para código de máquina.

Não se deve ignorar, porém, a distinção entre construtor e padrão, a
verificação de entrada completa, a estrutura da AST e os testes de precedência
e delimitadores. Esses itens definem o contrato que as fases futuras usarão.

### 5.3 Validação da estrutura proposta

A proposta passa por uma validação estrutural inicial se a equipe conseguir
demonstrar os seguintes invariantes:

1. Toda declaração `type` possui um nome e ao menos uma variante.
2. Cada variante possui nome único dentro do ADT.
3. Uma variante sem payload aparece sem argumentos.
4. Uma variante com payload informa a quantidade e os tipos dos campos.
5. Uma construção usa um construtor conhecido e o número correto de valores.
6. O scrutinee de `match` é uma expressão única entre parênteses ou, se os
  parênteses forem opcionais, a gramática define claramente onde ele termina.
7. Cada braço possui exatamente um padrão, `=>` e uma expressão resultante.
8. Todos os braços de um mesmo `match` produzem tipos compatíveis quando o
  type checker for implementado.
9. `_` não cria binding; `Some(v)` cria o binding `v` somente naquele braço.
10. O parser rejeita texto restante depois do programa.

Para a primeira entrega, os itens 1 a 7 devem ser representáveis na AST e
testáveis pelo parser. Os itens 8 e 9 podem ser implementados no type checker
depois, mas a modelagem escolhida não pode impedi-los. O item 10 é uma
propriedade de robustez do parser atual e deve ser corrigido desde já.

Testes mínimos derivados da proposta:

```text
type Color = Red | Green | Blue;
type Shape = Circle(int) | Rectangle(int, int);
type OptionInt = Some(int) | None;
match (shape) { Circle(r) => r; Rectangle(w, h) => w + h; }
match (color) { Red => 1; _ => 0; }
```

Também devem existir testes negativos para variante sem argumento recebendo
argumento, quantidade errada de payloads, `=>` ausente, braço sem expressão,
ADT sem variantes e lixo após a última declaração.

## 6. Alteração sugerida na AST

Uma evolução mínima em `src/ir/ast.rs` precisa representar quatro conceitos:

1. declaração do tipo algébrico;
2. construtor e seus campos;
3. construção de valor;
4. pattern matching e seus padrões.

Uma forma possível é:

```rust
pub struct AdtDecl {
    pub name: String,
    pub variants: Vec<VariantDecl>,
}

pub struct VariantDecl {
    pub name: String,
    pub fields: Vec<Param>,
}

pub struct MatchArm<Ty> {
    pub pattern: Pattern<Ty>,
    pub value: Box<ExprD<Ty>>,
}

pub enum Pattern<Ty> {
    Wildcard,
    Ident(String),
    Variant { name: String, fields: Vec<Pattern<Ty>> },
}
```

E as novas variantes mais naturais seriam:

```rust
pub enum Expr<Ty> {
    // variantes atuais ...
    Construct {
        name: String,
        args: Vec<ExprD<Ty>>,
    },
    Match {
        scrutinee: Box<ExprD<Ty>>,
        arms: Vec<MatchArm<Ty>>,
    },
}
```

O programa passaria a conter declarações além de funções, por exemplo:

```rust
pub struct Program<Ty> {
    pub types: Vec<AdtDecl>,
    pub functions: Vec<FunDecl<Ty>>,
}
```

Uma alternativa é colocar tipos e funções em um `Decl` comum. Para a primeira
entrega, `types` separado é mais simples de percorrer e reduz mudanças
acidentais na API existente.

### 6.1 O que deve e o que não deve ficar em `Type`

A representação mais direta do tipo algébrico é:

```rust
Type::Named(String)
```

ou, com mais informação:

```rust
Type::Adt {
    name: String,
    variants: Vec<VariantDecl>,
}
```

Recomendo `Type::Named(String)` na primeira entrega. A tabela de símbolos
resolverá o nome para a declaração completa. Duplicar a lista de variantes em
cada ocorrência de tipo torna igualdade, cópia e resolução mais difíceis.

Os construtores não devem ser tratados apenas como funções comuns: um
construtor precisa carregar sua identidade de variante e ser validado contra o
ADT correto. A AST pode usar `Construct`; a resolução da assinatura fica para
a semântica.

### 6.2 Padrão não é expressão

Não use `Expr` para representar um padrão. Uma expressão calcula um valor;
um padrão descreve uma forma e pode introduzir bindings. Separar `Pattern`
evita confundir:

- `Some(x)` no lado do padrão, que captura `x`;
- `Some(10)` como construção ou futuro padrão literal;
- `Some(expr)` como expressão de construção.

Esse é um ponto de projeto que merece ser defendido na entrega.

## 7. Alteração sugerida no parser

### 7.1 Parser de declarações de tipo

Adicionar em `src/parser/types.rs` ou em `functions.rs` um parser para:

```text
adt_decl     := 'type' identifier '{' variant* '}'
variant      := identifier ['(' param_list ')'] ';'
```

O parser de `param` já existe em `functions.rs`, mas está privado. Vale
extrair uma função reutilizável ou criar um parser específico de campos. Não
force uma dependência circular entre `types.rs`, `functions.rs` e
`statements.rs`.

Depois, em `program.rs`, substituir a ideia de apenas `many0(fun_decl)` por
uma sequência de declarações que aceite ADTs e funções. O parser deve
preservar a ordem para diagnósticos futuros, mesmo que a semântica faça duas
passagens.

### 7.2 Parser de construção

`atom` já reconhece chamadas com `parse_call`. Há duas opções:

- transformar toda chamada em `Call` e resolver construtores depois;
- reconhecer uma forma distinta e construir `Expr::Construct`.

A segunda opção é mais explícita e torna a AST autoexplicativa. Para não
duplicar gramática, `parse_call` pode retornar uma estrutura intermediária
com nome e argumentos, e `atom` pode decidir o nó final apenas se o parser
receber uma tabela de nomes. Como o parser atual não possui contexto, a opção
prática é aceitar sintaticamente `Name(args)` e fazer a classificação na
semântica, ou criar `Construct` para toda chamada em posição de expressão e
resolver chamadas de função separadamente.

Minha recomendação para a primeira entrega: adicionar `Expr::Construct` e
parsear construções com uma convenção lexical simples, como nomes de variante
iniciados por maiúscula. A validação de que o nome realmente é um construtor
fica para a próxima fase.

### 7.3 Parser de `match`

Adicionar `match` a `atom` ou `primary`:

```text
match_expr := 'match' expression '{' match_arm+ '}'
match_arm  := pattern '=>' expression ';'
pattern    := '_' | identifier | identifier '(' pattern_list ')'
```

Atenção: `expression` é gulosa e pode consumir o início do bloco se a
gramática não usar delimitadores claros. O parser deve consumir o scrutinee
até o `{` de abertura do match. Uma solução simples é manter a forma atual de
expressões, fazer `expression` parar naturalmente em `{` e testar casos como:

```c
match result { Some(value) => value; None => 0; }
```

Os parênteses do padrão devem chamar `pattern`, nunca `expression`.

### 7.4 Identificadores reservados

`src/parser/identifiers.rs` já rejeita palavras reservadas como `int`, `void`,
`true` e `false`. A extensão precisa incluir `type`, `match`, `Some` e `None`
conforme as decisões finais. Se variantes forem identificadores normais,
não reserve `Some` e `None`; prefira resolver seus papéis na tabela de nomes.

## 8. O que muda depois da primeira entrega

Mesmo que não seja cobrado agora, o impacto das novas construções deve ser
planejado.

### 8.1 Type checker

Em `src/semantic/type_checker.rs`, será necessário:

- registrar ADTs e variantes antes de verificar funções;
- resolver `Type::Named`;
- verificar quantidade e tipos dos argumentos de cada construtor;
- garantir que todos os braços de `match` retornem tipos compatíveis;
- inserir bindings dos campos capturados no ambiente de cada braço;
- rejeitar padrões impossíveis ou, no mínimo, detectar construtoras ausentes;
- decidir se `match` precisa ser exaustivo.

Para um compilador didático, match exaustivo é uma ótima meta: o conjunto de
variantes está disponível no ADT e pode ser comparado com os padrões usados.
O curinga `_` cobre as variantes restantes.

### 8.2 Interpretador

Em `src/interpreter/value.rs`, uma representação possível é:

```rust
Value::Adt {
    type_name: String,
    variant: String,
    fields: Vec<Value>,
}
```

`eval_expr` avalia `Construct`. Para `Match`, avalia o scrutinee, tenta os
padrões em ordem, cria os bindings capturados em um escopo temporário e
avalia a expressão do primeiro braço compatível.

A semântica deve documentar a ordem: em geral, o primeiro braço que casa é
escolhido. Um padrão depois de `_` pode ser considerado inalcançável numa
fase posterior.

### 8.3 TAC e alvo final

`src/ir/tac.rs` já possui saltos condicionais, labels, chamadas e temporários,
mas `src/codegen/tac_code_gen.rs` ainda tem vários `todo!()` e cobre apenas
parte da AST. Para ADTs, será preciso escolher uma representação de runtime:

- tag inteira para a variante;
- armazenamento dos payloads;
- convenção para acesso aos campos;
- estratégia de memória e alinhamento no alvo escolhido.

RISC-V é uma boa opção pedagógica porque torna visíveis chamadas, stack,
labels e layout de dados. WebAssembly reduz detalhes de hardware, mas exige
decisões próprias sobre memória linear. LLVM oferece infraestrutura poderosa,
mas aumenta a superfície conceitual e de dependências.

## 9. Problemas atuais e melhorias prioritárias

### Problemas de correção

1. **Entrada restante ignorada.** `main.rs` descarta o restante retornado por
   `program`. Use consumo total para não aceitar programas parciais.
2. **`many0` no topo.** Um erro em uma declaração pode virar parada silenciosa.
   Prefira uma combinação que exija EOF após declarações válidas.
3. **Palavras-chave sem fronteira lexical.** Parsers com `tag("if")`, por
   exemplo, devem evitar aceitar `ifdef` como `if` seguido de lixo.
4. **Parser e AST podem divergir.** Cada nova variante da AST exige atualização
   explícita nos matches do type checker, interpreter e codegen.
5. **TAC incompleto.** Não deve ser usado como evidência de que a geração final
   já funciona; há caminhos com `todo!()`.

### Melhorias de engenharia

- introduzir posições de origem (`span`) em tokens ou nós para erros úteis;
- criar um lexer pequeno se as fronteiras de palavras e diagnósticos crescerem;
- usar um tipo de erro de parser próprio, preservando contexto e posição;
- separar parsing de nomes de resolução de nomes;
- adicionar testes de AST, não apenas testes que verificam `is_err()`;
- testar entrada completa com `all_consuming`;
- documentar a gramática em uma notação única antes de codificar novas regras.

## 10. Plano de implementação da primeira entrega

### Etapa A: contrato da linguagem

1. Enviar ao professor a sintaxe proposta.
2. Fixar nomes (`type`, construtores, `match`, `=>`).
3. Fixar se construtores têm campos posicionais ou nomeados.
4. Fixar o conjunto inicial de padrões.

### Etapa B: AST

1. Adicionar `AdtDecl`, `VariantDecl`, `Pattern` e `MatchArm`.
2. Adicionar `Expr::Construct` e `Expr::Match`.
3. Adicionar ADTs ao `Program`.
4. Atualizar aliases e derives.
5. Fazer o projeto compilar, mesmo que semântica e execução ainda rejeitem os
   novos nós explicitamente.

### Etapa C: parser

1. Implementar parser de ADT isolado.
2. Implementar parser de padrões isolado.
3. Implementar parser de `match` isolado.
4. Integrar declarações ao parser de programa.
5. Adicionar testes de sucesso e falha para cada regra.

### Etapa D: validação da entrega

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

Também é importante verificar manualmente a AST produzida para:

```c
type OptionInt { None; Some(int value); }
```

e:

```c
match result { Some(value) => value; None => 0; }
```

O teste mais valioso não é apenas “parseou”: é confirmar que a árvore contém
`Program.types`, uma variante `Some` com campo `value`, `Expr::Match`, dois
braços e um `Pattern::Variant` com captura.

## 11. Perguntas que a equipe deve saber responder

- Qual é a diferença entre AST não tipada e AST decorada?
- Em qual nível da cadeia de precedência `match` deve entrar e por quê?
- Por que `Pattern` não deve ser modelado como `Expr`?
- Como o parser sabe que terminou o programa?
- O que acontece se dois ADTs possuírem uma variante com o mesmo nome?
- O construtor é uma função, um valor, ou uma entidade própria?
- Como detectar que um `match` não cobre todas as variantes?
- Como representar uma variante com payload no runtime?
- Como uma tag de variante seria traduzida para RISC-V, WebAssembly ou LLVM?

Essas perguntas conectam a implementação local aos conceitos de compiladores:
gramática, representação intermediária, análise semântica, escopo, controle de
fluxo e layout de valores.

## Conclusão

O repositório já oferece uma base adequada para o trabalho: a AST é pequena,
parametrizada por fase e consumida por componentes separados; o parser usa
combinators que espelham a gramática; e o runtime já usa enums Rust, uma
analogia concreta para tipos algébricos.

A primeira entrega deve ser tratada como a definição de um contrato. A AST
precisa expressar a nova linguagem sem ambiguidade, e o parser precisa
produzir exatamente essa representação. Type checker, interpretador e
geração de código podem vir depois, mas suas necessidades devem orientar a
modelagem desde agora.