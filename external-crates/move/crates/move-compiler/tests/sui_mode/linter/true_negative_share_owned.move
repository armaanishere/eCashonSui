// object is re shared, but it is never transferred and doesn't have public transfer
module a::is_not_transferred {
    use sui::transfer;
    use sui::tx_context::TxContext;
    use sui::object::UID;

    struct Obj has key {
        id: UID
    }

    public fun make_obj(ctx: &mut TxContext): Obj {
        Obj { id: sui::object::new(ctx) }
    }

    public fun crate(ctx: &mut TxContext) {
        transfer::share_object(make_obj(ctx));
    }

    public fun share(o: Obj) {
        transfer::share_object(o);
    }
}

// object is created locally, even though it is transferred somewhere else and has public share
module a::can_determine_to_be_new {
    use sui::transfer;
    use sui::object::UID;

    struct Obj has key, store {
        id: UID
    }

    fun make_obj(_: u64, _: vector<vector<u8>>, ctx: &mut sui::tx_context::TxContext): Obj {
        Obj { id: sui::object::new(ctx) }
    }

    public fun transfer(ctx: &mut sui::tx_context::TxContext) {
        let o = make_obj(0, vector[], ctx);
        transfer::transfer(o, @0);
    }

    public fun share(ctx: &mut sui::tx_context::TxContext) {
        let o = make_obj(0, vector[], ctx);
        transfer::share_object(o);
    }
}



module sui::tx_context {
    struct TxContext has drop {}
    public fun sender(_: &TxContext): address {
        @0
    }
}

module sui::object {
    struct UID has store {
        id: address,
    }
    public fun delete(_: UID) {
        abort 0
    }
    public fun new(_: &mut sui::tx_context::TxContext): UID {
        abort 0
    }
}

module sui::transfer {
    public fun transfer<T: key>(_: T, _: address) {
        abort 0
    }
    public fun share_object<T: key>(_: T) {
        abort 0
    }
    public fun public_share_object<T: key>(_: T) {
        abort 0
    }
}
