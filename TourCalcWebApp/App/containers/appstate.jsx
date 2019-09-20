import Cookies from 'js-cookie';


export default class AppState {
    state = {
        token: null,
        isMaster: null,
        allowedTourId: null,
        currentTour:  null,
        tours: null,
        isAuthLoaded: false,
        error: null,
        authData: null
    }

    token_name = '__tourcalc_token'

    static login(comp, scope, code) {
        let url = '/api/auth/token/' + scope + '/' + code;
        return fetch(url)
            .then((res) => res.text())
            .then((result) => {
//                alert('url: ' + url + ' res: ' + JSON.stringify(result, null, 2));
                //alert('url: ' + url + ' res: ' + result);
                Cookies.set('__tourcalc_token', result)
            })
            .then((result) => { AppState.checkWhoAmI(comp) })
    }

    static checkWhoAmI(comp) {
        this.token = Cookies.get('__tourcalc_token')
        //alert('token: ' + this.token)
        return fetch('/api/auth/whoami', {
            method: 'get',
            headers: new Headers({
                "Authorization": 'Bearer ' + this.token
            })
        })
        .then((res) => res.json())
        .then(
           (result) => {
               this.state = {
                   isAuthLoaded: true,
                   authData: result
               };
               comp.setState(this.state);
           },
           (error) => {
               this.state = {
                   isAuthLoaded: true,
                   error
               };
               comp.setState(this.state);
           })

    }


}