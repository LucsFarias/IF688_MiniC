# Sintaxe fechada — tipos algébricos e pattern matching (entrega 1)

Este documento é a **fonte única de verdade** para a sintaxe de `type` e
`match` na primeira entrega. Ele fecha as decisões deixadas em aberto em
`docs/10-guia-e-planejamento.md` (seção 5.1) e substitui a gramática que
aparecia em `docs/11-plano-entrega-adt.md`. Qualquer dúvida sobre "como essa
construção deve ser escrita" se resolve aqui, não durante a implementação.

O critério usado para fechar cada decisão foi **simplicidade**: entre duas
opções tecnicamente válidas, escolhemos a que exige menos regras de
gramática, menos tipos na AST e menos casos no type checker/interpretador
futuros.

## 1. Decisões e por quê

| Decisão | Escolhida | Por quê |
|---|---|---|
| Declaração de tipo | `type Nome = Variante \| Variante(...) \| ...;` | Deixa explícito que é soma de variantes; usa token já existente (`;`) para terminar |
| Payload da variante | **posicional**, só tipos: `Circle(int)`, `Rectangle(int, int)` | Sem posicional não precisa inventar nomes de campo, não precisa parser de `param`, e o padrão (`Circle(r)`) já é posicional — construtor e padrão casam campo a campo pela posição, sem tabela de nomes |
| Separador de braço do `match` | `;` | Mesmo token que já termina statements no Mini-C; evita introduzir `,` só para isso |
| Parênteses no scrutinee | **obrigatórios**: `match (e) { ... }` | `expression` é gulosa; sem um delimitador explícito antes do `{`, o parser pode tentar consumir o início do corpo do match como parte da expressão |
| Construtor vs. chamada de função | convenção léxica: identificador iniciado por **maiúscula** → construtor | Não exige tabela de símbolos disponível durante o parsing; `Some`, `None`, `Circle` são construtores só pela forma do nome |
| Stdlib / `printf` / `int main()` | **sem mudança** — continua `print` e `void main()` | ADT e pattern matching não têm relação com a API da stdlib nem com o contrato de `main`; misturar as duas coisas é escopo indevido |

## 2. Gramática (EBNF)

```ebnf
program    = { decl } EOF ;
decl       = adt_decl | fun_decl ;

adt_decl   = "type" ident "=" variant { "|" variant } ";" ;
variant    = ident [ "(" type_name { "," type_name } ")" ] ;

match_expr = "match" "(" expression ")" "{" match_arm+ "}" ;
match_arm  = pattern "=>" expression ";" ;

pattern    = "_"
           | lower_ident
           | upper_ident [ "(" pattern { "," pattern } ")" ] ;

construct  = upper_ident [ "(" expression { "," expression } ")" ] ;
```

Notas:

- `type_name` já existe em `src/parser/functions.rs` (`int`, `float`, `bool`,
  `str`, arrays, e agora também um nome de ADT — ver `Type::Named` abaixo).
- `lower_ident` / `upper_ident` são o `identifier` que já existe em
  `src/parser/identifiers.rs`, filtrado pela primeira letra. Não é um lexer
  novo, é uma verificação (`verify`) sobre o parser existente.
- `construct` sem parênteses (`None`, `Red`) é uma variante sem payload.
- Um padrão `Ident(...)` sem argumentos (`None`) e um padrão `lower_ident`
  (`x`) são gramaticalmente ambíguos só na aparência — a distinção é a
  maiúscula inicial, igual à de `construct`.

## 3. AST alvo

Esta é a forma exata que a Pessoa 1 implementa em `src/ir/ast.rs` e que todo
mundo mais programa contra (ver `docs/13-plano-trabalho-adt.md`):

```rust
pub enum Type {
    Unit, Int, Float, Bool, Str,
    Array(Box<Type>),
    Fun(Vec<Type>, Box<Type>),
    Any,
    Named(String),                                    // NOVO — nome de um ADT
}

pub struct AdtDecl {
    pub name: String,
    pub variants: Vec<VariantDecl>,
}

pub struct VariantDecl {
    pub name: String,
    pub fields: Vec<Type>,                             // posicional — sem nomes de campo
}

pub enum Pattern {                                     // não é Expr, não é parametrizado por Ty
    Wildcard,                                           // _
    Bind(String),                                       // x
    Variant { name: String, fields: Vec<Pattern> },     // Some(v) / None / Rectangle(w, h)
}

pub struct MatchArm<Ty> {
    pub pattern: Pattern,
    pub body: Box<ExprD<Ty>>,
}

pub enum Expr<Ty> {
    // ... variantes atuais (Literal, Ident, Add, Call, Index, ArrayLit, ...)
    Construct { name: String, args: Vec<ExprD<Ty>> },
    Match { scrutinee: Box<ExprD<Ty>>, arms: Vec<MatchArm<Ty>> },
}

pub struct Program<Ty> {
    pub types: Vec<AdtDecl>,
    pub functions: Vec<FunDecl<Ty>>,
}
```

Por que `Pattern` não é genérico em `Ty` nem reaproveita `Expr`: um padrão
não calcula valor, apenas descreve uma forma e opcionalmente cria bindings.
Ele não passa pela análise de tipos da mesma forma que uma expressão passa —
o tipo de cada campo capturado vem do `VariantDecl` correspondente, não de
uma inferência sobre o padrão em si. Misturar as duas coisas em `Expr`
obrigaria o type checker e o interpretador a rejeitar em runtime combinações
que deveriam ser impossíveis de escrever.

## 4. Exemplos válidos

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
    _ => 0;
  };
  print(value);
}
```

Isso corresponde à AST:

- `Program.types` com três `AdtDecl` (`Color`, `Shape`, `OptionInt`);
- `OptionInt.variants` = `[VariantDecl { name: "Some", fields: [Type::Int] }, VariantDecl { name: "None", fields: [] }]`;
- o `match` em `main` como `Expr::Match` com dois braços, o primeiro
  `Pattern::Variant { name: "Some", fields: [Pattern::Bind("v")] }`, o
  segundo `Pattern::Wildcard`.

## 5. Exemplos inválidos (devem ser rejeitados pelo parser)

```c
type Vazio = ;                          // ADT sem nenhuma variante
type Dup = A | A(int);                  // nome de variante repetido no mesmo ADT
None(1);                                // variante sem payload recebendo argumento
Rectangle(1);                           // aridade de payload errada (esperado 2)
match (s) { Circle(r) r * r; }          // braço sem "=>"
match (s) { Circle(r) => ; }            // braço sem expressão
match s { Circle(r) => r; }             // scrutinee sem parênteses — rejeitado pela gramática fechada
Some(1 + 1) => x;                       // dentro de um padrão, argumento não pode ser expressão arbitrária
type Cor = Vermelho | Verde
int x = 1;                              // falta ";" no fim do type — parser não deve aceitar o resto do arquivo como programa válido
```

O último caso é o mais importante de testar: hoje `src/main.rs` descarta o
que sobra da entrada depois de `program(&source)`, então um arquivo
parcialmente reconhecido pode passar como válido. Isso é corrigido junto com
esta entrega (ver `docs/13-plano-trabalho-adt.md`, seção da Pessoa 2) e deve
ser coberto por teste.

## 6. Fora de escopo nesta entrega

- tipos genéricos (`Option<T>`);
- payload nomeado nas variantes;
- variantes recursivas (uma lista que contém outra lista);
- padrões literais (`Some(0)`), guards (`Some(v) if v > 0`) e desestruturação de arrays;
- verificação de exaustividade do `match`;
- `match` como statement (nesta entrega ele só existe como `Expr::Match`);
- qualquer mudança na stdlib (`printf`, formatação, etc.);
- representação em memória para RISC-V/WebAssembly/LLVM.

## 7. Palavras reservadas novas

`type` e `match` entram em `RESERVED` em `src/parser/identifiers.rs`. `Some`,
`None`, `Red` etc. **não** são reservadas — são identificadores comuns que a
convenção da maiúscula inicial classifica como construtor no ponto de uso.
