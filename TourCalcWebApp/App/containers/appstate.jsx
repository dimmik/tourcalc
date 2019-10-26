import Cookies from 'js-cookie';


export default class AppState {
    state = {
        token: null,
        isMaster: null,
        isTourLoaded: null,
        tour:  null,
        tours: null,
        isAuthLoaded: false,
        error: null,
        authData: null,
        isToursLoaded: false,
        tours: null
    }


    static login(comp, scope, code) {
        let url = '/api/auth/token/' + scope + '/' + code;
        return fetch(url)
            .then((res) => res.text())
            .then((result) => {
                //alert('url: ' + url + ' res: ' + JSON.stringify(result, null, 2));
                //alert('url: ' + url + ' res: ' + result);
                Cookies.set('__tourcalc_token', result, { expires: 180 })
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
                alert('Error loading auth info')

               this.state = {
                   isAuthLoaded: true,
                   error
               };
               comp.setState(this.state);
           })

    }

    static addPerson(comp, tourid, person) {
        return fetch(
            '/api/tour/' + tourid + '/person', {
                method: 'post',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                }),
                body: JSON.stringify(person, null, 2)
            }
        )
            .then((res) => res.text())
            .then((res) => res, (error) => { alert('Error add person') })

    }
    static editPerson(comp, tourid, person) {
        return fetch(
            '/api/tour/' + tourid + '/person/' + person.guid, {
                method: 'PATCH',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                }),
                body: JSON.stringify(person, null, 2)
            }
        )
            .then((res) => res.text())
            .then((res) => res, (error) => { alert('Error edit person') })

    }
    static deletePerson(comp, tourid, guid) {
        return fetch(
            '/api/tour/' + tourid + '/person/' + guid, {
                method: 'DELETE',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                })
            }
        )
            .then((res) => res.text())
            .then((res) => res, (error) => { alert('Error delete person') })

    }

    static addSpending(comp, tourid, spending) {
        spending.planned = false;
        return fetch(
            '/api/tour/' + tourid + '/spending', {
                method: 'post',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                }),
                body: JSON.stringify(spending, null, 2)
            }
        )
            .then((res) => res.text())
            .then((res) => res, (error) => { alert('Error add add spending') })

    }
    static editSpending(comp, tourid, spending) {
        return fetch(
            '/api/tour/' + tourid + '/spending/' + spending.guid, {
                method: 'PATCH',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                }),
                body: JSON.stringify(spending, null, 2)
            }
        )
            .then((res) => res.text())
            .then((res) => res, (error) => { alert('Error edit spending') })

    }
    static deleteSpending(comp, tourid, guid) {
        return fetch(
            '/api/tour/' + tourid + '/spending/' + guid, {
                method: 'DELETE',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                })
            }
        )
            .then((res) => res.text())
            .then((res) => res, (error) => { alert('Error delete spending') })

    }

    static loadTour(comp, tourid) {
//        return fetch('/api/tour/' + tourid + '/calculated', {
        return fetch('/api/tour/' + tourid + '/suggested', {
            method: 'get',
            headers: new Headers({
                "Authorization": 'Bearer ' + this.token
            })
        })
            .then((res) => res.json())
            .then(
                (result) => {
                    this.state = {
                        isTourLoaded: true,
                        tour: result,
                        updateTime: new Date()
                    };
                    comp.setState(this.state);
                },
            (error) => {
                    //alert('Error loading tour')
                    this.state = {
                        isTourLoaded: true,
                        error
                    };
                    comp.setState(this.state);
                })
    }

    static loadTours(comp, from, count) {
        if (from == null) from = 0;
        if (count == null) count = 50;
        let url = '/api/tour/all/suggested?from='+from+'&count='+count;
        return fetch(url, {
            method: 'get',
            headers: new Headers({
                "Authorization": 'Bearer ' + this.token
            })
        })
            .then(res => res.json())
            .then(result => {
                this.state = {
                    isToursLoaded: true,
                    tours: result
                };
                comp.setState(this.state)
            }, error => {
                alert("error loading tours")
                this.state = {
                    isToursLoaded: true,
                    error
                };
                comp.setState(this.state)
            })
    }
    static addTour(comp, tname, accessCode) {
        let b = JSON.stringify({ name: tname }, null, 2)
        //alert('b: ' + b)
        return fetch('/api/tour/add/' + accessCode, {
            method: 'post',
            headers: new Headers({
                "Authorization": 'Bearer ' + this.token,
                "Content-Type": "application/json"
            }),
            body: b
        })
            .then((res) => {
                if (res.status != 200) throw new Error(res.statusText)
                return res.text()
            })
            .then((res) => res, (error) => { alert('Not added: ' + error) })
    }
    static addTourJson(comp, tour, accessCode, tname) {
        if (tname != null) {
            tour.name = tname
        }
        let b = JSON.stringify(tour, null, 2)
        //alert('b: ' + b)
        return fetch('/api/tour/add/' + accessCode, {
            method: 'post',
            headers: new Headers({
                "Authorization": 'Bearer ' + this.token,
                "Content-Type": "application/json"
            }),
            body: b
        })
            .then(res => {

                if (res.status != 200) throw new Error(res.statusText)
                    return res.text()
                }
            )
            .then((res) => res, (error) => { alert('Not added: ' + error) })
    }
    static changeTourName(comp, tourid, tname, tcode) {
        let b = JSON.stringify({ name: tname, AccessCodeMD5: tcode }, null, 2)
        //alert('b: ' + b)
        return fetch('/api/tour/' + tourid + "/changename", {
            method: 'PATCH',
            headers: new Headers({
                "Authorization": 'Bearer ' + this.token,
                "Content-Type": "application/json"
            }),
            body: b
        })
            .then(res => res.text())
            .then((res) => res, (error) => { alert('Error change tour name') })

    }
    static deleteTour(comp, tourid) {
        return fetch(
            '/api/tour/' + tourid, {
                method: 'DELETE',
                headers: new Headers({
                    "Authorization": 'Bearer ' + this.token,
                    "Content-Type": "application/json"
                })
            }
        )
            .then((res) => {
                if (res.status != 200) throw new Error(res.statusText)
                return res.text()
            })
            .then((res) => res, (error) => { alert('Not deleted: ' + error) })

    }
}