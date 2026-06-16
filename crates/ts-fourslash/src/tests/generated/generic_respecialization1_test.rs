#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_respecialization1() {
    let mut t = TestingT;
    run_test_generic_respecialization1(&mut t);
}

fn run_test_generic_respecialization1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericRespecialization1") {
        return;
    }
    let content = r#"// @strict: false
class Food {
    private amount: number;
    constructor(public name: string) {
        this.amount = 100;
    }
    public eat(amountToEat: number): boolean {
        this.amount -= amountToEat;
        if (this.amount <= 0) {
            this.amount = 0;
            return false;
        }
        else {
            return true;
        }
    }
}
class IceCream extends Food {
    private isDairyFree: boolean;
    constructor(public flavor: string) {
        super("Ice Cream");
    }
}
class Cookie extends Food {
    constructor(public flavor: string, public isGlutenFree: boolean) {
        super("Cookie");
    }
}
class Slug {
    // This is NOT a food!!!
}
class GenericMonster<T extends Food, V> {
    private name: string;
    private age: number;
    private isFriendly: boolean;
    constructor(name: string, age: number, isFriendly: boolean, private food: T, public variant: V) {
        this.name = name;
        this.age = age;
        this.isFriendly = isFriendly;
    }
    public getFood(): T {
        return this.food;
    }
    public getVariant(): V {
        return this.variant;
    }
    public eatFood(amountToEat: number): boolean {
        return this.food.eat(amountToEat);
    }
    public sayGreeting(): string {
        return ("My name is " + this.name + ", and my age is " + this.age + ".  I enjoy eating " + this.food.name + " and my variant is " + this.variant);
    }
}
class GenericPlanet<T extends GenericMonster</*2*/Cookie, any>> {
    constructor(public name: string, public solarSystem: string, public species: T) { }
}
var cookie = new Cookie("Chocolate Chip", false);
var cookieMonster = new GenericMonster<Cookie, string>("Cookie Monster", 50, true, cookie, "hello");
var sesameStreet = new GenericPlanet<GenericMonster<Cookie, string>>("Sesame Street", "Alpha Centuri", cookieMonster);
class GenericPlanet2<T extends Food, V>{
    constructor(public name: string, public solarSystem: string, public species: GenericMonster<T, V>) { }
}
 /*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.insert_line(t, "");
    f.verify_no_errors();
    f.go_to_marker(t, "2");
    f.delete_at_caret(t, 6);
    f.insert(t, "any");
    f.verify_no_errors();
    f.insert_line(t, "var narnia = new GenericPlanet2<Cookie, string>(");
    done();
}
