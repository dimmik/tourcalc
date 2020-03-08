import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import { BrowserRouter as Router, Route, Switch, Redirect  } from 'react-router-dom';
import LoginScreen from './login-screen.jsx';
import TourList from './tour-list.jsx'
import TourUI from './tour-ui.jsx'

export default class App extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            key: 0
        }
    }
    componentDidMount() {
        //alert('app mounted');
        AppState.setMainApp(this);
    }
    
    render() {
        return (
            <Router key={this.state.key}>
                <div>
                    <main>
                    <Switch>
                            <Route path="/access/:scope/:code"
                                render={(props) => (<RequestAccessCode app={this} scope={props.match.params.scope} code={props.match.params.code} />)} />
                            <Route path="/goto/:code/:tourid"
                                render={(props) => (<RequestAccessCode app={this} scope="code" code={props.match.params.code} tourid={props.match.params.tourid}/>)} />
                            <Route path="/tour/:tourid"
                                render={(props) => (<TourUI app={this} tourid={props.match.params.tourid} />)} />
                            <Route path="/login" render={(props) => <LoginScreen app={this}/>} />
                            <Route path="/" component={Index} />
                    </Switch>
                    </main>
                    </div>
            </Router>
            )
    }
    
};

class RequestAccessCode extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            redirect: false
        }
    }
    
    componentDidMount() {
        AppState.login(this.props.app, this.props.scope, this.props.code)
            .then(() => { this.setState({redirect: true}) })
    }

    render() {
        //alert("tid: " + this.props.tourid);
        if (this.state.redirect) {
            if (this.props.tourid != null) {
                return <Redirect to={"/tour/" + this.props.tourid} />
            } else {
                return <Redirect to="/" />
            }
        } else {
            return <div>Checking Access Code</div>
        }
    }
}
class Index extends React.Component {
    constructor(props) {
        super(props);
        this.state = {}
    }
    componentDidMount() {
        AppState.checkWhoAmI(this);
    }

    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (this.state.authData.type === "None") {
                return (<Redirect to="/login"/>)
            } else {
                return (
                    <TourList app={this} authData={this.state.authData} />
                )
            }
        }
    }

}
ReactDOM.render(
    <App />,
    document.getElementById("content")
);