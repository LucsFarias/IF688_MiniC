# Plano de trabalho — entrega 1 (AST e parser para ADTs e pattern matching)

Divide o trabalho entre 4 pessoas. Pressupõe que `docs/12-sintaxe-adt.md` já
foi lido e aceito por todos — qualquer dúvida sobre "como se escreve" se
resolve lá, não neste documento. Este documento só trata de "quem implementa
o quê, em qual arquivo, e em qual ordem".

As referências `arquivo:linha` correspondem ao estado atual do repositório
(antes desta entrega) e vão se mover assim que o código for editado — usem
como ponto de partida.

## 1. Resumo de responsabilidades

| Pessoa | Entrega | Não é responsável por |
|---|---|---|
| 1 | AST nova em `src/ir/ast.rs` + manter o projeto compilando | Revisar PRs das pessoas 2, 3 e 4; `tac_code_gen.rs` |
| 2 | Parser de `type ... = ...;`, integração de declarações em `program.rs`, correção de consumo total da entrada | Parser de `match` ou de `Construct` |
| 3 | Parser de padrões e da expressão `match` | Registrar palavras reservadas (feito junto com a Pessoa 2, ver seção 4) |
| 4 | Parser de `Construct`, suíte de testes de aceitação, **portão de validação final** (`cargo fmt`/`test`/`clippy`) | Definir a AST (isso é da Pessoa 1) |

O ponto que muda em relação à primeira conversa: a **revisão final e o
portão de validação são da Pessoa 4**, não da Pessoa 1. A Pessoa 1 já carrega
o trabalho bloqueante (seção 2) e não deve acumular também a checagem de
saída do time.

## 2. Sequenciamento

```
Dia 0        Leitura de docs/12-sintaxe-adt.md por todos. Branch feat/adt-parser criada.
Dia 1–2      Pessoa 1: AST + build verde                              [BLOQUEIA 2, 3 e 4]
Dia 2–5      Pessoas 2, 3 e 4 em paralelo, cada uma com testes isolados antes de integrar
Dia 5–6      Integração em program.rs (Pessoa 2) e em atom() (Pessoas 3 e 4, coordenado)
Dia 6–7      Pessoa 4 roda o portão de validação e a suíte de aceitação; ajustes finais coletivos
```

As pessoas 2, 3 e 4 mexem em arquivos majoritariamente diferentes
(`types.rs` novo, `patterns.rs`/`match_expr` novo, `Construct` em
`expressions.rs`), então o paralelismo do Dia 2–5 tem pouco conflito de
merge. O ponto de atrito é `atom()` em `expressions.rs` (seção 5 e 6 abaixo).

---

## 3. Pessoa 1 — AST e manter o build verde

**Bloqueia as outras três pessoas.** Meta: sair em 1–2 dias.

### 3.1 Adicionar os tipos novos

Em `src/ir/ast.rs`, exatamente como especificado em `docs/12-sintaxe-adt.md`
seção 3: `Type::Named` (novo variant no enum de `src/ir/ast.rs:53`),
`AdtDecl`, `VariantDecl`, `Pattern`, `MatchArm<Ty>`, `Expr::Construct` e
`Expr::Match` (novos variants no enum `Expr<Ty>` de `src/ir/ast.rs:85`), e
`Program.types` (`src/ir/ast.rs:172`, hoje só tem `functions`). Manter
`#[derive(Debug, Clone, PartialEq)]` em cada tipo novo, igual aos existentes.

### 3.2 Resolver os pontos que quebram

Assim que `Program` e `Expr` mudam de forma, os seguintes pontos param de
compilar (matches exaustivos ou construção de `Program` sem o campo novo):

| Arquivo:linha | Problema | Correção |
|---|---|---|
| `src/parser/program.rs` (`Program { functions }`) | falta `types` | placeholder `Program { types: Vec::new(), functions }` — a Pessoa 2 substitui essa função inteira depois |
| `src/semantic/type_checker.rs:120` (`Ok(Program { functions })`) | idem | passe-direto real: `Ok(Program { types: program.types.clone(), functions })` |
| `src/semantic/type_checker.rs:345` (`type_check_expr_inner`, match exaustivo em `Expr<()>`) | não cobre `Construct`/`Match` | dois braços novos: `Expr::Construct { .. } => Err(TypeError::new("adt/match not yet supported"))` e o mesmo para `Expr::Match { .. }` |
| `src/semantic/type_checker.rs:422` (`type_check_expr`, match exaustivo em `&e.exp`) | idem | mesmos dois braços, retornando `Err(TypeError::new(...))` |
| `src/interpreter/eval_expr.rs:49` (`eval_expr`, match exaustivo em `&expr.exp`, termina em `Expr::Call` na linha 145) | idem | dois braços novos antes do fechamento do match, retornando `Err(RuntimeError::new(...))` |

**Não mexer em `src/codegen/tac_code_gen.rs`.** Os `match` ali já terminam em
`_ => todo!()`, então cobrem as variantes novas sem alteração — confirmar com
`cargo build`, mas não editar esse arquivo.

### 3.3 Critério de pronto

- `cargo build` limpo.
- `cargo test` continua passando sem alteração (nenhum teste hoje monta
  `Program { .. }` por struct-literal fora do parser).
- PR mergeado na branch de integração antes de 2, 3 e 4 abrirem os parsers.


---

## 4. Pessoa 2 — parser de declaração de tipo e robustez do topo

**Depende de:** Pessoa 1 mergeada (`AdtDecl`, `VariantDecl`, `Program.types`).

### 4.1 Arquivo novo: `src/parser/types.rs`

```rust
pub fn variant(input: &str) -> IResult<&str, VariantDecl> { ... }
pub fn adt_decl(input: &str) -> IResult<&str, AdtDecl> { ... }
```

`variant` usa `type_name` (já público em `src/parser/functions.rs:38`) para
os tipos do payload — como o payload é posicional (`docs/12`, seção 1), não
é preciso reaproveitar o parser `param` (que produz `(nome, tipo)`); só uma
lista de `type_name` separada por vírgula.

Registrar o módulo em `src/parser/mod.rs` (`pub mod types;` + reexport de
`adt_decl`, seguindo o padrão dos outros `pub use`).

### 4.2 Integrar ao programa: `src/parser/program.rs`

Hoje `program` é só `map(many0(fun_decl), |functions| Program { functions })`
(`src/parser/program.rs`, função `program`). Substituir por uma alternância
entre declaração de tipo e função, preservando a ordem do arquivo fonte:

```rust
enum Decl { Adt(AdtDecl), Fun(UncheckedFunDecl) }

fn decl(input: &str) -> IResult<&str, Decl> {
    alt((map(adt_decl, Decl::Adt), map(fun_decl, Decl::Fun)))(input)
}

pub fn program(input: &str) -> IResult<&str, UncheckedProgram> {
    map(many0(preceded(multispace0, decl)), |decls| {
        let mut types = Vec::new();
        let mut functions = Vec::new();
        for d in decls {
            match d {
                Decl::Adt(a) => types.push(a),
                Decl::Fun(f) => functions.push(f),
            }
        }
        Program { types, functions }
    })(input)
}
```

### 4.3 Corrigir o consumo total da entrada

`src/main.rs:32` chama `program(&source)` e descarta o resto
(`Ok((_, prog)) =>` ignora o primeiro elemento da tupla, que é o restante não
consumido). Trocar por `all_consuming(program)(&source)`
(`nom::combinator::all_consuming`), para que um arquivo com lixo no final
vire erro em vez de programa parcial aceito silenciosamente. Isso corrige o
problema 9.1 do guia (`docs/10-guia-e-planejamento.md`) e é testável
diretamente com o último exemplo inválido de `docs/12-sintaxe-adt.md` seção 5.

### 4.4 Palavras reservadas

Em `src/parser/identifiers.rs:31` (array `RESERVED`), adicionar `"type"` e
**`"match"` no mesmo PR**, mesmo sendo usado só pela Pessoa 3 — é uma linha
só, e dois PRs tocando a mesma linha em paralelo é conflito garantido. A
Pessoa 3 não deve editar essa linha de novo.

### 4.5 Testes antes de integrar

`adt_decl` isolado com os casos de `docs/12-sintaxe-adt.md` seção 4 e 5:
`Color` (só variantes sem payload), `Shape`/`OptionInt` (payload posicional),
e os negativos — ADT sem variantes, nome de variante repetido, vírgula
sobrando, `;` faltando no fim.

---

## 5. Pessoa 3 — parser de padrões e expressão `match`

**Depende de:** Pessoa 1 mergeada (`Pattern`, `MatchArm`, `Expr::Match`);
`"match"` já reservado (Pessoa 2, seção 4.4 — não editar `RESERVED` de novo).

### 5.1 Arquivo novo: `src/parser/patterns.rs`

```rust
fn pattern(input: &str) -> IResult<&str, Pattern> {
    alt((
        map(preceded(multispace0, char('_')), |_| Pattern::Wildcard),
        map(variant_pattern, |(name, fields)| Pattern::Variant { name, fields }),
        map(preceded(multispace0, identifier), |s: &str| Pattern::Bind(s.to_string())),
    ))(input)
}

fn variant_pattern(input: &str) -> IResult<&str, (String, Vec<Pattern>)> {
    let (rest, name) = preceded(multispace0, identifier)(input)?;
    let (rest, fields) = delimited(
        preceded(multispace0, char('(')),
        separated_list0(preceded(multispace0, char(',')), pattern),
        preceded(multispace0, char(')')),
    )(rest)?;
    Ok((rest, (name.to_string(), fields)))
}
```

`variant_pattern` precisa vir **antes** de `Bind` no `alt`: como
`variant_pattern` falha de forma limpa quando não há `(` depois do nome, a
ordem acima já garante que `Some(v)` vira `Variant`/`Some(v)` seja tentado
antes de qualquer coisa cair em `Bind`. Os parênteses do padrão chamam
**`pattern`, nunca `expression`** — é o ponto de design de `docs/12`, seção 3
(padrão não é expressão).

### 5.2 Expressão `match`

```rust
fn match_arm(input: &str) -> IResult<&str, MatchArm<()>> {
    let (rest, pat) = preceded(multispace0, pattern)(input)?;
    let (rest, _) = preceded(multispace0, tag("=>"))(rest)?;
    let (rest, body) = preceded(multispace0, expression)(rest)?;
    let (rest, _) = preceded(multispace0, char(';'))(rest)?;
    Ok((rest, MatchArm { pattern: pat, body: Box::new(body) }))
}

pub fn match_expr(input: &str) -> IResult<&str, UncheckedExpr> {
    let (rest, _) = preceded(multispace0, tag("match"))(input)?;
    let (rest, scrutinee) = delimited(
        preceded(multispace0, char('(')),
        preceded(multispace0, expression),
        preceded(multispace0, char(')')),
    )(rest)?;
    let (rest, arms) = delimited(
        preceded(multispace0, char('{')),
        many1(preceded(multispace0, match_arm)),
        preceded(multispace0, char('}')),
    )(rest)?;
    Ok((rest, wrap(Expr::Match { scrutinee: Box::new(scrutinee), arms })))
}
```

Os parênteses obrigatórios no scrutinee (`docs/12`, seção 1) evitam o
problema de `expression` gulosa engolir o `{` de abertura do match — não é
preciso nenhuma lógica extra de lookahead por causa dessa decisão.

### 5.3 Integração em `src/parser/expressions.rs`

`atom()` (linha 69) hoje é:

```rust
fn atom(input: &str) -> IResult<&str, UncheckedExpr> {
    alt((
        map(literal, |l| wrap(Expr::Literal(l.into()))),
        map(parse_call, |(name, args)| wrap(Expr::Call { name, args })),
        map(/* array literal */),
        map(identifier, |s: &str| wrap(Expr::Ident(s.to_string()))),
        delimited(/* parênteses */),
    ))(input)
}
```

Adicionar `map(match_expr, |e| e)` como a **primeira** alternativa. Como
`"match"` já está em `RESERVED`, não há colisão com `identifier` nem com
`parse_call`, então a posição exata entre as primeiras alternativas não
quebra nada — mas ir primeiro deixa a intenção clara na leitura do código.

### 5.4 Testes antes de integrar

`pattern` isolado (`_`, `x`, `Some(v)`, `Rectangle(w, h)`). `match_expr`
isolado com os exemplos de `docs/12-sintaxe-adt.md` seção 4. Negativos:
braço sem `=>`, braço sem `;`, scrutinee sem parênteses, padrão tentando usar
uma expressão (`Some(1 + 1)` deve falhar — está fora de escopo por
`docs/12`, seção 6).

---

## 6. Pessoa 4 — construtores, suíte de aceitação e portão de validação

**Depende de:** Pessoa 1 mergeada (`Expr::Construct`). Coordena com a Pessoa
3 quem faz o merge final de `atom()` (seção 6.2).

### 6.1 Convenção de maiúscula em `src/parser/expressions.rs`

```rust
fn upper_ident(input: &str) -> IResult<&str, &str> {
    verify(identifier, |s: &str| {
        s.chars().next().is_some_and(|c| c.is_uppercase())
    })(input)
}

fn construct(input: &str) -> IResult<&str, UncheckedExpr> {
    let (rest, name) = preceded(multispace0, upper_ident)(input)?;
    let (rest, args) = opt(delimited(
        preceded(multispace0, char('(')),
        separated_list0(preceded(multispace0, char(',')), expression),
        preceded(multispace0, char(')')),
    ))(rest)?;
    Ok((rest, wrap(Expr::Construct {
        name: name.to_string(),
        args: args.unwrap_or_default(),
    })))
}
```

### 6.2 Ordem crítica em `atom()`

`construct` precisa ser tentado **antes** de `parse_call` e de `identifier`
(`src/parser/expressions.rs:69-90`). Nem `parse_call` nem `identifier`
filtram por caixa — sem essa ordem, `Some(42)` seria reconhecido como
`Expr::Call { name: "Some", .. }` em vez de `Expr::Construct`. Este é o
segundo ponto de coordenação de ordem dentro do mesmo `alt` (o primeiro é o
`match_expr` da Pessoa 3, seção 5.3) — combinem antecipadamente quem faz o
PR que edita `atom()` por último, para não terem dois PRs reescrevendo a
mesma função em paralelo.

### 6.3 Suíte de testes de aceitação (`tests/adt.rs`, arquivo novo)

Testes de **forma da AST**, não só `is_err()`/`is_ok()`. No mínimo, os dois
exemplos canônicos de `docs/12-sintaxe-adt.md` seção 4:

- `type OptionInt = Some(int) | None;` → `Program.types[0].variants` contém
  `Some` com `fields: [Type::Int]` e `None` com `fields: []`.
- `match (a) { Some(v) => v; _ => 0; }` → `Expr::Match` com 2 braços, o
  primeiro `Pattern::Variant { name: "Some", fields: [Pattern::Bind("v")] }`,
  o segundo `Pattern::Wildcard`.

Mais os negativos consolidados de `docs/12-sintaxe-adt.md` seção 5 — um teste
por caso, cobrindo o que 2 e 3 já testaram isoladamente mais os de
integração (declaração de tipo seguida de função, `match` dentro de `return`,
lixo após a última declaração do programa).

### 6.4 Portão de validação final 

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

Antes de rodar `clippy -- -D warnings`, verificar se já existem warnings de
`dead_code` pré-existentes em `src/codegen/tac_code_gen.rs` (não relacionados
a esta entrega). Se existirem, resolver com `#[allow(dead_code)]` nas
funções afetadas — mudança de uma linha, segura e reversível — em vez de
deixar o portão falhar por algo fora do escopo do ADT.

Esta etapa é o critério de "pronto para entregar": só a Pessoa 4 assina que o
portão passou. Isso substitui a ideia original de a Pessoa 1 revisar tudo no
final — ela já fechou a AST no início e não deve ser o gargalo de saída
também.

---

## 7. Perguntas de defesa (distribuir 2–3 por pessoa)

- Qual é a diferença entre AST não tipada e AST decorada?
- Em qual nível da cadeia de precedência `match` entra e por quê?
- Por que `Pattern` não deve ser modelado como `Expr`?
- Como o parser sabe que terminou o programa?
- O que acontece se dois ADTs tiverem uma variante com o mesmo nome?
- O construtor é uma função, um valor, ou uma entidade própria?
- Como detectar que um `match` não cobre todas as variantes?
- Como representar uma variante com payload no runtime?
- Como uma tag de variante seria traduzida para RISC-V, WebAssembly ou LLVM?
