# Tourcalc
Tool to track budget of a trip, tour or party.  

## Get started (listen on 8080, using docker image):
  
```
podman run -d -p 127.0.0.1:8080:80 --env-file ~/config/tourcalc.env --name tourcalc-blazor ghcr.io/dimmik/tourcalc:blazor-latest
```

where `~/config/tourcalc.env` is like
```
MasterKey={admin password}
StorageType={MongoDb, InMemory}
MongoDbUsername=mongo
MongoDbUrl={mongo db url}
MongoDbPassword={mongo db passwd}
AuthPrivateECDSAKey={random key, for example rczPGwhweUcW9JjQPLoV/xHE9/ennHbwpAUpwkBmbHE=}

```

### example (mongodb):
```
MasterKey=123456
StorageType=MongoDb
MongoDbUsername=mongo
MongoDbUrl=some.azure.mongodb.net
MongoDbPassword=654321
AuthPrivateECDSAKey=rczPGwhweUcW9JjQPLoV/xHE9/ennHbwpAUpwkBmbHE=
```

### example (non-persistent in memory storage):
```
MasterKey=123456
StorageType=InMemory
AuthPrivateECDSAKey=rczPGwhweUcW9JjQPLoV/xHE9/ennHbwpAUpwkBmbHE=
```

### About MasterKey

The Master key is for admin access. 
Only admin can create first tour in a so-called "code", only admin can delete last one in the code.
And some other functionality is limited to admin.

In react (`ghcr.io/dimmik/tourcalc:react-latest`) it should be entered on login page with scope = admin

In blazor (`ghcr.io/dimmik/tourcalc:blazor-latest`) it entered on login page as `admin:{MasterKey}`, like `admin:123456`


