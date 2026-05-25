use crate::lexer::{Keyword, Punct, TokenKind};

use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

impl<'a> Parser<'a> {
    pub(crate) fn parse_item(&mut self) {
        match self.peek().kind() {
            TokenKind::Keyword(Keyword::Pub) => self.parse_pub_item(),
            TokenKind::Keyword(Keyword::Module) => self.parse_module_decl(),
            TokenKind::Keyword(Keyword::Use) => self.parse_use_decl(),
            TokenKind::Keyword(Keyword::Data) => self.parse_data_decl(),
            TokenKind::Keyword(Keyword::Layout) => self.parse_layout_data_decl(),
            TokenKind::Keyword(Keyword::Class) => self.parse_class_decl(),
            TokenKind::Keyword(Keyword::Unique) => self.parse_unique_class_decl(),
            TokenKind::Keyword(Keyword::Interface) => self.parse_interface_decl(),
            TokenKind::Keyword(Keyword::Error) => self.parse_error_decl(),
            TokenKind::Keyword(Keyword::Image) => self.parse_image_decl(),
            TokenKind::Keyword(Keyword::Host) => self.parse_host_image_decl(),
            _ => self.parse_error_item(),
        }
    }

    fn parse_pub_item(&mut self) {
        self.start_node(SyntaxKind::PublicItem);
        self.start_node(SyntaxKind::PubModifier);
        self.bump();
        self.finish_node();
        self.parse_item();
        self.finish_node();
    }

    fn parse_data_decl(&mut self) {
        self.start_node(SyntaxKind::DataDecl);
        self.bump();
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_field_block();
        self.finish_node();
    }

    fn parse_layout_data_decl(&mut self) {
        self.start_node(SyntaxKind::LayoutDataDecl);
        self.bump(); // layout
        self.expect_identifier(); // layout ABI, such as C
        self.expect_keyword(Keyword::Data, "expected data after layout");
        self.expect_identifier();
        self.parse_field_block();
        self.finish_node();
    }

    fn parse_interface_decl(&mut self) {
        self.start_node(SyntaxKind::InterfaceDecl);
        self.bump();
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_braced("expected '{'", "expected '}'", |parser| {
            parser.parse_method_signature_decl();
        });
        self.finish_node();
    }

    fn parse_error_decl(&mut self) {
        self.start_node(SyntaxKind::ErrorDecl);
        self.bump();
        self.expect_identifier();
        self.parse_field_block();
        self.finish_node();
    }

    fn parse_unique_class_decl(&mut self) {
        self.start_node(SyntaxKind::UniqueClassDecl);
        self.bump();
        self.expect_keyword(Keyword::Class, "expected class after unique");
        self.parse_class_tail();
        self.finish_node();
    }

    fn parse_class_decl(&mut self) {
        self.start_node(SyntaxKind::ClassDecl);
        self.bump();
        self.parse_class_tail();
        self.finish_node();
    }

    fn parse_class_tail(&mut self) {
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_implements_clause();
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        self.parse_list_until(Punct::CloseBrace, "expected '}'", |parser| {
            parser.parse_member();
        });
    }

    fn parse_implements_clause(&mut self) {
        if self.eat_keyword(Keyword::Implements) {
            self.start_node(SyntaxKind::ImplementsClause);
            self.parse_type_ref();
            while self.eat_punct(Punct::Comma) {
                self.parse_type_ref();
            }
            self.finish_node();
        }
    }

    fn parse_field_block(&mut self) {
        self.parse_braced("expected '{'", "expected '}'", |parser| {
            parser.parse_field_decl();
        });
    }

    fn parse_field_decl(&mut self) {
        self.start_node(SyntaxKind::FieldDecl);
        self.expect_binding_name();
        self.expect_punct(Punct::Colon, "expected ':'");
        self.parse_type_ref();
        self.finish_node();
    }

    fn parse_member(&mut self) {
        match self.peek().kind() {
            TokenKind::Keyword(Keyword::Constructor) => self.parse_constructor_decl(),
            TokenKind::Keyword(Keyword::Fn) | TokenKind::Keyword(Keyword::Asm) => {
                self.parse_method_decl()
            }
            TokenKind::Keyword(Keyword::Test) => self.parse_test_decl(),
            TokenKind::Identifier if self.peek_n(1).kind() == TokenKind::Punct(Punct::Colon) => {
                self.parse_field_decl()
            }
            _ => self.parse_member_error(),
        }
    }

    fn parse_method_decl(&mut self) {
        self.start_node(SyntaxKind::MethodDecl);
        self.parse_method_head();
        self.parse_block();
        self.finish_node();
    }

    fn parse_constructor_decl(&mut self) {
        self.start_node(SyntaxKind::ConstructorDecl);
        self.bump();
        self.parse_param_list();
        if self.eat_punct(Punct::Arrow) {
            self.parse_return_type();
        }
        self.parse_block();
        self.finish_node();
    }

    fn parse_test_decl(&mut self) {
        self.start_node(SyntaxKind::TestDecl);
        self.bump();
        if self.peek().kind() == TokenKind::StringLiteral {
            self.bump();
        } else {
            self.expect_identifier();
        }
        self.parse_block();
        self.finish_node();
    }

    fn parse_phase_decl(&mut self) {
        self.start_node(SyntaxKind::PhaseDecl);
        self.bump();
        self.expect_identifier();
        self.parse_param_list();
        self.parse_block();
        self.finish_node();
    }

    fn parse_method_signature_decl(&mut self) {
        self.start_node(SyntaxKind::MethodDecl);
        self.parse_method_head();
        self.finish_node();
    }

    fn parse_method_head(&mut self) {
        let _ = self.eat_keyword(Keyword::Asm);
        self.expect_keyword(Keyword::Fn, "expected item");
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_param_list();
        if self.eat_punct(Punct::Arrow) {
            self.parse_return_type();
        }
    }

    fn parse_return_type(&mut self) {
        self.start_node(SyntaxKind::ReturnType);
        self.parse_type_ref();
        self.finish_node();
    }

    fn parse_param_list(&mut self) {
        self.start_node(SyntaxKind::ParamList);
        self.expect_punct(Punct::OpenParen, "expected '('");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseParen)
            && self.peek().kind() != TokenKind::Eof
        {
            self.start_node(SyntaxKind::Param);
            if matches!(
                self.peek().kind(),
                TokenKind::Keyword(Keyword::Read)
                    | TokenKind::Keyword(Keyword::Mut)
                    | TokenKind::Keyword(Keyword::Own)
            ) {
                self.bump();
            }
            self.expect_binding_name();
            if self.eat_punct(Punct::Colon) {
                self.parse_type_ref();
            }
            self.finish_node();
            if !self.eat_punct(Punct::Comma) {
                break;
            }
        }
        self.expect_close_punct(Punct::CloseParen, "expected ')'");
        self.finish_node();
    }

    fn parse_image_decl(&mut self) {
        self.start_node(SyntaxKind::ImageDecl);
        self.bump();
        self.expect_identifier();
        self.expect_keyword(Keyword::Target, "expected target in image declaration");
        self.parse_type_ref();
        self.parse_image_body();
        self.finish_node();
    }

    fn parse_host_image_decl(&mut self) {
        self.start_node(SyntaxKind::HostImageDecl);
        self.bump();
        self.expect_keyword(Keyword::Image, "expected image after host");
        self.expect_identifier();
        self.parse_image_body();
        self.finish_node();
    }

    fn parse_image_body(&mut self) {
        self.parse_braced("expected '{'", "expected '}'", |parser| {
            if parser.peek().kind() == TokenKind::Keyword(Keyword::Phase) {
                parser.parse_phase_decl();
            } else {
                parser.parse_image_body_error();
            }
        });
    }

    fn parse_image_body_error(&mut self) {
        self.start_node(SyntaxKind::RecoveryNode);
        self.error_at_current(SyntaxErrorKind::UnexpectedToken, "unexpected token");
        self.bump();
        self.consume_to_image_body_boundary();
        self.finish_node();
    }

    fn parse_member_error(&mut self) {
        self.start_node(SyntaxKind::RecoveryNode);
        self.error_at_current(
            SyntaxErrorKind::UnexpectedToken,
            "unexpected token in class body",
        );
        self.bump();
        self.consume_to_member_boundary();
        self.finish_node();
    }

    fn parse_module_decl(&mut self) {
        self.start_node(SyntaxKind::ModuleDecl);
        self.bump();
        self.parse_module_path();
        self.finish_node();
    }

    fn parse_use_decl(&mut self) {
        self.start_node(SyntaxKind::UseDecl);
        self.bump();
        if self.eat_punct(Punct::OpenBrace) {
            self.start_node(SyntaxKind::UseBinderList);
            if self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) {
                self.parse_use_binder();
                while self.eat_punct(Punct::Comma) {
                    if self.peek().kind() == TokenKind::Punct(Punct::CloseBrace) {
                        break;
                    }
                    self.parse_use_binder();
                }
            }
            self.finish_node();
            self.expect_close_punct(Punct::CloseBrace, "expected '}'");
        } else {
            let span = self.peek().span();
            self.diagnostic(
                SyntaxErrorKind::ExpectedImportBinderList,
                span,
                "expected import binder list",
            );
            self.builder
                .error(SyntaxErrorKind::ExpectedImportBinderList, span);
        }
        if self.eat_keyword(Keyword::From) {
            self.parse_module_path_after_from();
        } else {
            let span = self.peek().span();
            self.diagnostic(
                SyntaxErrorKind::ExpectedFrom,
                span,
                "expected from in use import",
            );
            self.builder.error(SyntaxErrorKind::ExpectedFrom, span);
            if self.peek().kind() == TokenKind::Identifier {
                self.parse_module_path();
            }
        }
        self.finish_node();
    }

    fn parse_use_binder(&mut self) {
        self.start_node(SyntaxKind::UseBinder);
        if self.peek().kind() == TokenKind::Punct(Punct::Star) {
            let span = self.peek().span();
            self.diagnostic(
                SyntaxErrorKind::InvalidImportBinder,
                span,
                "wildcard imports are not supported in v1",
            );
            self.builder
                .error(SyntaxErrorKind::InvalidImportBinder, span);
            self.bump();
        } else if self.expect_identifier() && self.eat_keyword(Keyword::As) {
            let span = self.peek().span();
            self.diagnostic(
                SyntaxErrorKind::InvalidImportBinder,
                span,
                "import aliases are not supported in v1",
            );
            self.builder
                .error(SyntaxErrorKind::InvalidImportBinder, span);
            self.expect_identifier();
        }
        self.finish_node();
    }

    fn parse_module_path_after_from(&mut self) {
        if self.peek().kind() != TokenKind::Identifier {
            let span = self.peek().span();
            self.diagnostic(
                SyntaxErrorKind::ExpectedModulePath,
                span,
                "expected module path after from",
            );
            self.builder
                .error(SyntaxErrorKind::ExpectedModulePath, span);
            return;
        }
        self.parse_module_path();
    }

    fn parse_module_path(&mut self) {
        self.start_node(SyntaxKind::ModulePath);
        self.expect_identifier();
        while self.eat_punct(Punct::Dot) {
            self.expect_identifier();
        }
        self.finish_node();
    }

    pub(crate) fn parse_error_item(&mut self) {
        self.start_node(SyntaxKind::RecoveryNode);
        self.error_at_current(SyntaxErrorKind::ExpectedItem, "expected item");
        self.bump();
        self.consume_to_item_boundary();
        self.finish_node();
    }
}
