# Sintaxe de ADTs e `match`

Este documento descreve a sintaxe e as decisões semânticas atualmente
implementadas para declarações de tipos algébricos, seus construtores e
statements `match`.

## 1. Decisões

| Decisão | Regra |
|---|---|
| Declaração ADT | `type Nome = Variante \| Variante(...);` |
| Payload | Posicional, declarado com tipos, por exemplo `Circle(int)` ou `Rectangle(int, int)`. |
| Construtor | Identificador iniciado por maiúscula: `Some(42)` ou `None`. |
| Scrutinee | Parênteses obrigatórios: `match (valor) { ... }`. |
| Corpo do braço | Sempre um bloco `{ ... }`, sem ponto e vírgula após o `}`. |
| Cobertura | O type checker exige cobertura exaustiva, inclusive dentro de padrões ADT aninhados. |
| Escopo de bindings | Os nomes capturados por um padrão existem somente no bloco daquele braço. |

## 2. Gramática

```ebnf
program    = { decl } EOF ;
decl       = adt_decl | fun_decl ;

adt_decl   = "type" ident "=" variant { "|" variant } ";" ;
variant    = ident [ "(" type_name { "," type_name } ")" ] ;

match_stmt = "match" "(" expression ")" "{" match_arm+ "}" ;
match_arm  = pattern "=>" block ;
block      = "{" statement* "}" ;

pattern    = "_"
           | lower_ident
           | upper_ident [ "(" pattern { "," pattern } ")" ] ;

construct  = upper_ident [ "(" expression { "," expression } ")" ] ;
```

Cada braço é separado do próximo pelo fechamento do seu bloco; não há `;`
depois do bloco. Cada statement dentro do bloco mantém seus próprios
terminadores.

`_` ignora o valor, um identificador minúsculo captura o valor inteiro, e um
identificador maiúsculo seleciona uma variante. Campos de variantes e
padrões são posicionais. Variantes sem payload podem ser escritas sem
parênteses.

## 3. Exemplos válidos

```c
type Color = Red | Green | Blue;
type Shape = Circle(int) | Rectangle(int, int);
type OptionInt = Some(int) | None;

void print_color(Color color) {
  match (color) {
    Red => { print("vermelho"); }
    Green => { print("verde"); }
    Blue => { print("azul"); }
  }
}

int area(Shape shape) {
  int result = 0;
  match (shape) {
    Circle(radius) => { result = radius * radius; }
    Rectangle(width, height) => { result = width * height; }
  }
  return result;
}

void main() {
  OptionInt value = Some(42);
  match (value) {
    Some(number) => { print(number); }
    None => { print(0); }
  }
}
```

Um `match` não é uma expressão: não pode inicializar uma variável, ser
retornado ou compor uma expressão. Os braços podem executar múltiplos
statements, inclusive `return`.

Padrões aninhados são verificados recursivamente:

```c
type Inner = Left(int) | Right;
type Outer = Wrapped(Inner) | Empty;

void inspect(Outer value) {
  match (value) {
    Wrapped(Left(number)) => { print(number); }
    Wrapped(Right) => {}
    Empty => {}
  }
}
```

## 4. AST

`Pattern` descreve a forma testada e não é uma expressão. A AST representa o
`match` em `Statement`, e o corpo de cada braço é um bloco de statements:

```rust
pub enum Pattern {
    Wildcard,
    Bind(String),
    Variant { name: String, fields: Vec<Pattern> },
}

pub struct MatchArm<Ty> {
    pub pattern: Pattern,
    pub body: Box<StatementD<Ty>>,
}

pub enum Statement<Ty> {
    // ...
    Match {
        scrutinee: Box<ExprD<Ty>>,
        arms: Vec<MatchArm<Ty>>,
    },
}
```

## 5. Erros detectados

O parser rejeita declarações sem variantes, variantes duplicadas no mesmo
ADT, padrões malformados e corpos de braço que não sejam blocos. O type
checker rejeita tipos e variantes desconhecidos, aridade ou tipo de payload
incorretos, variantes que pertencem a outro ADT, bindings duplicados,
scrutinees que não sejam ADTs e matches não exaustivos.

```c
match (value) { Some(number) => { print(number); } } // não cobre None
match (value) { Some => {} None => {} }              // aridade incorreta
match (value) { Some(number) => print(number); }     // corpo não é bloco
```

Uma alternativa `_` ou um binding que capture o valor inteiro torna a
cobertura exaustiva. A verificação também considera os campos de padrões
aninhados.

## 6. Fora de escopo

- padrões literais (`Some(0)`), guards e desestruturação de arrays;
- geração TAC de construtores ADT e `match` (o backend atual é parcial);
- representação de ADTs em backends de máquina, RISC-V, WebAssembly ou LLVM;
- mudanças na API da biblioteca padrão.
